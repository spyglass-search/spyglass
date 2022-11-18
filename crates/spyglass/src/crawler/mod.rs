use std::collections::HashSet;
use std::path::Path;

use addr::parse_domain_name;
use anyhow::Result;
use chrono::prelude::*;
use chrono::Duration;
use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::{Host, Url};

use entities::models::{crawl_queue, fetch_history, tag};
use entities::sea_orm::prelude::*;

use crate::connection::load_connection;
use crate::crawler::bootstrap::create_archive_url;
use crate::parser;
use crate::scraper::{html_to_text, DEFAULT_DESC_LENGTH};
use crate::state::AppState;

pub mod bootstrap;
pub mod client;
pub mod robots;

use client::HTTPClient;
use robots::check_resource_rules;

// TODO: Make this configurable by domain
const FETCH_DELAY_MS: i64 = 1000 * 60 * 60 * 24;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CrawlError {
    #[error("crawl denied by rule {0}")]
    Denied(String),
    #[error("unable crawl document due to {0}")]
    FetchError(String),
    #[error("unable to parse document due to {0}")]
    ParseError(String),
    /// Document was not found.
    #[error("document not found")]
    NotFound,
    #[error("document was recently fetched")]
    RecentlyFetched,
    /// Request timeout, crawler will try again later.
    #[error("document request timed out")]
    Timeout,
    #[error("crawl unsupported: {0}")]
    Unsupported(String),
    #[error("other crawl error: {0}")]
    Other(String),
}

#[derive(Debug, Default, Clone)]
pub struct CrawlResult {
    /// Used to determine
    pub content_hash: Option<String>,
    /// Text content from page after stripping HTML tags & any semantically
    /// unimportant sections (header/footer/etc.)
    pub content: Option<String>,
    /// A short description of the page provided by the <meta> tag or summarized
    /// from the content.
    pub description: Option<String>,
    pub title: Option<String>,
    /// Uniquely identifying URL for this document. Used by the crawler to determine
    /// duplicates & how/what to crawl
    pub url: String,
    /// URL used to open the document in finder/web browser/etc.
    pub open_url: Option<String>,
    /// Links found in the page to add to the queue.
    pub links: HashSet<String>,
    /// Tags to apply to this document
    pub tags: Vec<tag::Model>,
}

impl CrawlResult {
    pub fn new(
        url: &Url,
        open_url: Option<String>,
        content: &str,
        title: &str,
        desc: Option<String>,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash = Some(hex::encode(&hasher.finalize()[..]));
        log::trace!("content hash: {:?}", content_hash);
        // Use a portion of the content
        let desc = if let Some(desc) = desc {
            Some(desc)
        } else {
            Some(content.to_string())
        };

        Self {
            content_hash,
            content: Some(content.to_string()),
            description: desc,
            title: Some(title.to_string()),
            url: url.to_string(),
            open_url,
            ..Default::default()
        }
    }
}

fn normalize_href(url: &str, href: &str) -> Option<String> {
    // Force HTTPS, crawler will fallback to HTTP if necessary.
    if let Ok(url) = Url::parse(url) {
        if href.starts_with("//") {
            // schema relative url
            if let Ok(url) = Url::parse(&format!("{}:{}", "https", href)) {
                return Some(url.to_string());
            }
        } else if href.starts_with("http://") || href.starts_with("https://") {
            // Force HTTPS, crawler will fallback to HTTP if necessary.
            if let Ok(url) = Url::parse(href) {
                let mut url = url;
                if url.scheme() == "http" {
                    url.set_scheme("https").expect("Unable to set HTTPS scheme");
                }
                return Some(url.to_string());
            }
        } else {
            // origin or directory relative url
            if let Ok(url) = url.join(href) {
                return Some(url.to_string());
            }
        }
    }

    log::debug!("Unable to normalize href: {} - {}", url.to_string(), href);
    None
}

#[derive(Debug, Clone)]
pub struct Crawler {
    pub client: HTTPClient,
}

impl Default for Crawler {
    fn default() -> Self {
        Self::new()
    }
}

fn determine_canonical(original: &Url, extracted: &Url) -> String {
    // Ignore IPs
    let origin_dn = match original.host() {
        Some(Host::Domain(s)) => Some(s),
        _ => None,
    };

    let extracted_dn = match extracted.host() {
        Some(Host::Domain(s)) => Some(s),
        _ => None,
    };

    if origin_dn.is_none() || extracted_dn.is_none() {
        return original.to_string();
    }

    // Only allow overrides on the same root domain.
    let origin_dn = parse_domain_name(origin_dn.expect("origin_dn should not be None"));
    let extracted_dn = parse_domain_name(extracted_dn.expect("extracted_dn should not be None"));

    if origin_dn.is_err() || extracted_dn.is_err() {
        return original.to_string();
    }

    let origin_dn = origin_dn.expect("origin_dn invalid");
    let extracted_dn = extracted_dn.expect("extracted_dn invalid");

    // Special case for bootstrapper.
    if let Some(root) = origin_dn.root() {
        if root == "archive.org" || Some(root) == extracted_dn.root() {
            return extracted.to_string();
        }
    }

    original.to_string()
}

impl Crawler {
    pub fn new() -> Self {
        Crawler {
            client: HTTPClient::new(),
        }
    }

    /// Fetches and parses the content of a page.
    async fn crawl(&self, url: &Url, parse_results: bool) -> Result<CrawlResult, CrawlError> {
        let url = url.clone();

        // Fetch & store page data.
        let res = self.client.get(&url).await;
        if res.is_err() {
            let err = res.unwrap_err();
            // Log out reason for failure.
            log::warn!("Unable to fetch <{}> due to {}", &url, err.to_string());
            // Unable to connect to host
            return Err(CrawlError::FetchError(err.to_string()));
        }

        let res = res.expect("Expected valid response");
        match res.error_for_status() {
            Ok(res) => {
                // Pull URL from request, this handles cases where we are 301 redirected
                // to a different URL.
                let end_url = res.url().to_owned();
                match res.text().await {
                    Ok(raw_body) => {
                        if parse_results {
                            Ok(self.scrape_page(&end_url, &raw_body).await)
                        } else {
                            Ok(CrawlResult {
                                url: end_url.to_string(),
                                open_url: Some(end_url.to_string()),
                                ..Default::default()
                            })
                        }
                    }
                    Err(err) => Err(CrawlError::ParseError(err.to_string())),
                }
            }
            Err(err) => {
                if err.status() == Some(reqwest::StatusCode::NOT_FOUND) {
                    Err(CrawlError::NotFound)
                } else {
                    Err(CrawlError::FetchError(err.to_string()))
                }
            }
        }
    }

    pub async fn scrape_page(&self, url: &Url, raw_body: &str) -> CrawlResult {
        // TODO: Cache the raw_body on the filesystem?

        // Parse the html.
        let parse_result = html_to_text(raw_body);

        // Hash the body content, used to detect changes (eventually).
        let mut hasher = Sha256::new();
        hasher.update(parse_result.content.as_bytes());
        let content_hash = Some(hex::encode(&hasher.finalize()[..]));
        log::trace!("content hash: {:?}", content_hash);

        let canonical_url = match parse_result.canonical_url {
            Some(canonical) => determine_canonical(url, &canonical),
            None => url.to_string(),
        };

        CrawlResult {
            content_hash,
            content: Some(parse_result.content),
            description: Some(parse_result.description),
            title: parse_result.title,
            url: canonical_url.clone(),
            open_url: Some(canonical_url),
            links: parse_result.links,
            ..Default::default()
        }
    }

    // TODO: Load web indexing as a plugin?
    /// Attempts to crawl a job from the crawl_queue specific by <id>
    /// * Checks whether we can crawl using any saved rules or looking at the robots.txt
    /// * Fetches & parses the page
    pub async fn fetch_by_job(
        &self,
        state: &AppState,
        id: i64,
        parse_results: bool,
    ) -> Result<CrawlResult, CrawlError> {
        let crawl = crawl_queue::Entity::find_by_id(id).one(&state.db).await;
        let crawl = match crawl {
            Ok(c) => c,
            Err(err) => {
                return Err(CrawlError::Other(err.to_string()));
            }
        };

        let crawl = match crawl {
            None => {
                return Err(CrawlError::Other("crawl job not found".to_string()));
            }
            Some(c) => c,
        };

        log::debug!("handling job: {}", crawl.url);

        let url = Url::parse(&crawl.url).expect("Invalid fetch URL");

        // Have we crawled this recently?
        if let Ok(Some(history)) = fetch_history::find_by_url(&state.db, &url).await {
            let since_last_fetch = Utc::now() - history.updated_at;
            if since_last_fetch < Duration::milliseconds(FETCH_DELAY_MS) {
                log::trace!("Recently fetched, skipping");
                return Err(CrawlError::RecentlyFetched);
            }
        }

        // Route URL to the correct fetcher
        // TODO: Have plugins register for a specific scheme and have the plugin
        // handle any fetching/parsing.
        match url.scheme() {
            "api" => self.handle_api_fetch(state, &crawl, &url).await,
            "file" => self.handle_file_fetch(&crawl, &url).await,
            "http" | "https" => {
                self.handle_http_fetch(&state.db, &crawl, &url, parse_results)
                    .await
            }
            // unknown scheme, ignore
            scheme => {
                log::warn!("Ignoring unhandled scheme: {}", &url);
                Err(CrawlError::Unsupported(scheme.to_string()))
            }
        }
    }

    async fn handle_api_fetch(
        &self,
        state: &AppState,
        _: &crawl_queue::Model,
        uri: &Url,
    ) -> Result<CrawlResult, CrawlError> {
        let account = percent_decode_str(uri.username()).decode_utf8_lossy();
        let api_id = uri.host_str().unwrap_or_default();

        match load_connection(state, api_id, &account).await {
            Ok(mut conn) => conn.as_mut().get(uri).await,
            Err(err) => Err(CrawlError::Unsupported(format!("{}: {}", api_id, err))),
        }
    }

    async fn handle_file_fetch(
        &self,
        _: &crawl_queue::Model,
        url: &Url,
    ) -> Result<CrawlResult, CrawlError> {
        // Attempt to convert from the URL to a file path
        let file_path = url.to_file_path().expect("Invalid file URL");

        let path = Path::new(&file_path);
        // Is this a file and does this exist?
        if !path.exists() || !path.is_file() {
            return Err(CrawlError::NotFound);
        }

        let file_name = path
            .file_name()
            .and_then(|x| x.to_str())
            .map(|x| x.to_string())
            .expect("Unable to convert path file name to string");

        // Attempt to read file
        let contents = match path.extension() {
            Some(ext) if parser::supports_filetype(ext) => match parser::parse_file(ext, path) {
                Err(err) => return Err(CrawlError::ParseError(err.to_string())),
                Ok(contents) => contents,
            },
            _ => match std::fs::read_to_string(path) {
                Ok(x) => x,
                Err(err) => {
                    return Err(CrawlError::FetchError(err.to_string()));
                }
            },
        };

        let mut hasher = Sha256::new();
        hasher.update(contents.as_bytes());
        let content_hash = Some(hex::encode(&hasher.finalize()[..]));

        // TODO: Better description building for text files?
        let description = if !contents.is_empty() {
            let desc = contents
                .split(' ')
                .into_iter()
                .take(DEFAULT_DESC_LENGTH)
                .collect::<Vec<&str>>()
                .join(" ");
            Some(desc)
        } else {
            None
        };

        Ok(CrawlResult {
            content_hash,
            content: Some(contents.clone()),
            // Does a file have a description? Pull the first part of the file
            description,
            title: Some(file_name),
            url: url.to_string(),
            open_url: Some(url.to_string()),
            links: Default::default(),
            ..Default::default()
        })
    }

    /// Handle HTTP related requests
    async fn handle_http_fetch(
        &self,
        db: &DatabaseConnection,
        crawl: &crawl_queue::Model,
        url: &Url,
        parse_results: bool,
    ) -> Result<CrawlResult, CrawlError> {
        // Modify bootstrapped URLs to pull from the Internet Archive
        let url: Url = if crawl.crawl_type == crawl_queue::CrawlType::Bootstrap {
            Url::parse(&create_archive_url(url.as_ref())).expect("Unable to create archive URL")
        } else {
            url.clone()
        };

        // Check for robots.txt of this domain
        // When looking at bootstrapped tasks, check the original URL
        if crawl.crawl_type == crawl_queue::CrawlType::Bootstrap {
            let og_url = Url::parse(&crawl.url).expect("Invalid crawl URL");
            if !check_resource_rules(db, &self.client, &og_url).await {
                return Err(CrawlError::Denied("robots.txt".to_string()));
            }
        } else if !check_resource_rules(db, &self.client, &url).await {
            return Err(CrawlError::Denied("robots.txt".to_string()));
        }

        // Crawl & save the data
        match self.crawl(&url, parse_results).await {
            Err(err) => {
                log::debug!("issue fetching {:?} - {}", url, err.to_string());
                Err(err)
            }
            Ok(mut result) => {
                log::debug!("fetched og: {}, canonical: {}", url, result.url);

                // Check to see if a canonical URL was found, if not use the original
                // bootstrapped URL
                if crawl.crawl_type == crawl_queue::CrawlType::Bootstrap {
                    let parsed = Url::parse(&result.url).expect("Invalid result URL");
                    let domain = parsed.host_str().expect("Invalid result URL host");
                    if domain == "web.archive.org" {
                        result.url = crawl.url.clone();
                    }
                }

                // Normalize links from scrape result. If the links start with "/" they
                // should be appended to the current URL.
                let normalized_links = result
                    .links
                    .iter()
                    .filter_map(|link| normalize_href(&result.url, link))
                    .collect();
                result.links = normalized_links;

                log::trace!(
                    "crawl result: {:?} - {:?}\n{:?}",
                    result.title,
                    result.url,
                    result.description,
                );

                // Update fetch history
                // Break apart domain + path of the URL
                let url = Url::parse(&result.url).expect("Invalid result URL");
                let domain = url.host_str().expect("Invalid URL");
                let mut path: String = url.path().to_string();
                if let Some(query) = url.query() {
                    path = format!("{}?{}", path, query);
                }

                let _ = fetch_history::upsert(db, domain, &path, result.content_hash.clone(), 200)
                    .await;

                Ok(result)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use entities::models::crawl_queue::CrawlType;
    use entities::models::{crawl_queue, resource_rule};
    use entities::sea_orm::{ActiveModelTrait, Set};
    use entities::test::setup_test_db;
    use spyglass_plugin::utils::path_to_uri;

    use crate::crawler::{determine_canonical, normalize_href, Crawler};
    use crate::state::AppState;
    use std::path::Path;
    use url::Url;

    #[tokio::test]
    #[ignore]
    async fn test_crawl() {
        let crawler = Crawler::new();
        let url = Url::parse("https://oldschool.runescape.wiki").unwrap();
        let result = crawler.crawl(&url, true).await.expect("success");

        assert_eq!(result.title, Some("Old School RuneScape Wiki".to_string()));
        assert_eq!(result.url, "https://oldschool.runescape.wiki/".to_string());
        assert!(result.links.len() > 0);
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch() {
        let crawler = Crawler::new();

        let db = setup_test_db().await;
        let url = Url::parse("https://oldschool.runescape.wiki/").unwrap();
        let query = crawl_queue::ActiveModel {
            domain: Set(url.host_str().unwrap().to_owned()),
            url: Set(url.to_string()),
            ..Default::default()
        };
        let model = query.insert(&db).await.unwrap();
        let state = AppState::builder().with_db(db).build();

        let result = crawler.fetch_by_job(&state, model.id, true).await.unwrap();
        assert_eq!(result.title, Some("Old School RuneScape Wiki".to_string()));
        assert_eq!(result.url, "https://oldschool.runescape.wiki/".to_string());

        let links: Vec<String> = result.links.into_iter().collect();
        assert!(links[0].starts_with("https://oldschool.runescape.wiki"));
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_redirect() {
        let crawler = Crawler::new();
        let db = setup_test_db().await;
        let state = AppState::builder().with_db(db).build();

        let url = Url::parse("https://xkcd.com/1375").unwrap();
        let query = crawl_queue::ActiveModel {
            domain: Set(url.host_str().unwrap().to_owned()),
            url: Set(url.to_string()),
            ..Default::default()
        };
        let model = query.insert(&state.db).await.unwrap();

        let result = crawler.fetch_by_job(&state, model.id, true).await.unwrap();
        assert_eq!(result.title, Some("xkcd: Astronaut Vandalism".to_string()));
        assert_eq!(result.url, "https://xkcd.com/1375/".to_string());
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_bootstrap() {
        let crawler = Crawler::new();
        let db = setup_test_db().await;
        let state = AppState::builder().with_db(db).build();

        let url = Url::parse("https://www.ign.com/wikis/luigis-mansion").unwrap();
        let query = crawl_queue::ActiveModel {
            domain: Set(url.host_str().unwrap().to_owned()),
            url: Set(url.to_string()),
            crawl_type: Set(CrawlType::Bootstrap),
            ..Default::default()
        };
        let model = query.insert(&state.db).await.unwrap();

        let result = crawler.fetch_by_job(&state, model.id, true).await.unwrap();
        assert_eq!(
            result.title,
            Some("Luigi's Mansion Wiki Guide - IGN".to_string())
        );
        assert_eq!(
            result.url,
            "https://www.ign.com/wikis/luigis-mansion/".to_string()
        );

        let links: Vec<String> = result.links.into_iter().collect();
        for link in links {
            assert!(!link.starts_with("https://web.archive.org"));
        }
    }

    #[tokio::test]
    async fn test_fetch_skip() {
        let crawler = Crawler::new();

        let db = setup_test_db().await;
        let state = AppState::builder().with_db(db).build();

        // Should skip this URL
        let url =
            Url::parse("https://oldschool.runescape.wiki/w/Worn_Equipment?veaction=edit").unwrap();
        let query = crawl_queue::ActiveModel {
            domain: Set(url.host_str().unwrap().to_owned()),
            url: Set(url.to_string()),
            crawl_type: Set(crawl_queue::CrawlType::Bootstrap),
            ..Default::default()
        };
        let model = query.insert(&state.db).await.unwrap();

        // Add resource rule to stop the crawl above
        let rule = resource_rule::ActiveModel {
            domain: Set("oldschool.runescape.wiki".into()),
            rule: Set("/.*\\?veaction=.*".into()),
            no_index: Set(false),
            allow_crawl: Set(false),
            ..Default::default()
        };
        let _ = rule.insert(&state.db).await.unwrap();

        let res = crawler.fetch_by_job(&state, model.id, true).await;
        assert!(res.is_err());
    }

    #[test]
    fn test_normalize_href() {
        let url = "https://example.com";

        assert_eq!(
            normalize_href(&url, "http://foo.com"),
            Some("https://foo.com/".into())
        );
        assert_eq!(
            normalize_href(&url, "https://foo.com"),
            Some("https://foo.com/".into())
        );
        assert_eq!(
            normalize_href(&url, "//foo.com"),
            Some("https://foo.com/".into())
        );
        assert_eq!(
            normalize_href(&url, "/foo.html"),
            Some("https://example.com/foo.html".into())
        );
        assert_eq!(
            normalize_href(&url, "/foo"),
            Some("https://example.com/foo".into())
        );
        assert_eq!(
            normalize_href(&url, "foo.html"),
            Some("https://example.com/foo.html".into())
        );
    }

    #[test]
    fn test_determine_canonical() {
        // Test a correct override
        let a = Url::parse("https://commons.wikipedia.org").unwrap();
        let b = Url::parse("https://en.wikipedia.org").unwrap();

        let res = determine_canonical(&a, &b);
        assert_eq!(res, "https://en.wikipedia.org/");

        // Test a valid override from a different domain.
        let a = Url::parse("https://web.archive.org").unwrap();
        let b = Url::parse("https://en.wikipedia.org").unwrap();

        let res = determine_canonical(&a, &b);
        assert_eq!(res, "https://en.wikipedia.org/");

        // Test ignoring an invalid override
        let a = Url::parse("https://localhost:5000").unwrap();
        let b = Url::parse("https://en.wikipedia.org").unwrap();

        let res = determine_canonical(&a, &b);
        assert_eq!(res, "https://localhost:5000/");

        // Test ignoring an invalid override
        let a = Url::parse("https://en.wikipedia.org").unwrap();
        let b = Url::parse("https://spam.com").unwrap();

        let res = determine_canonical(&a, &b);
        assert_eq!(res, "https://en.wikipedia.org/");
    }

    #[tokio::test]
    async fn test_file_fetch() {
        let crawler = Crawler::new();

        let db = setup_test_db().await;
        let state = AppState::builder().with_db(db).build();

        #[cfg(target_os = "windows")]
        let test_folder = Path::new("C:\\tmp\\path_to_uri");
        #[cfg(not(target_os = "windows"))]
        let test_folder = Path::new("/tmp/path_to_uri");

        std::fs::create_dir_all(test_folder).expect("Unable to create test dir");

        let test_path = test_folder.join("test.txt");
        std::fs::write(test_path.clone(), "test_content").expect("Unable to write test file");

        let uri = path_to_uri(test_path.to_path_buf());
        let url = Url::parse(&uri).unwrap();

        let query = crawl_queue::ActiveModel {
            domain: Set("localhost".to_string()),
            url: Set(url.to_string()),
            crawl_type: Set(crawl_queue::CrawlType::Bootstrap),
            ..Default::default()
        };
        let model = query.insert(&state.db).await.unwrap();

        // Add resource rule to stop the crawl above
        let res = crawler.fetch_by_job(&state, model.id, true).await;
        assert!(res.is_ok());
    }
}
