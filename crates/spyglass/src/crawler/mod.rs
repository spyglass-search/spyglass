use addr::parse_domain_name;
use anyhow::Result;
use chrono::prelude::*;
use chrono::Duration;
use entities::models::tag::TagPair;
use entities::models::{crawl_queue, fetch_history};
use entities::sea_orm::prelude::*;
use governor::clock::QuantaClock;
use governor::state::keyed::DashMapStateStore;
use governor::Quota;
use governor::RateLimiter;
use libnetrunner::crawler::handle_crawl;
use libnetrunner::parser::html::{html_to_text, DEFAULT_DESC_LENGTH};
use nonzero_ext::nonzero;
use percent_encoding::percent_decode_str;
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::num::NonZeroU32;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use url::{Host, Url};

use crate::connection::load_connection;
use crate::crawler::bootstrap::create_archive_url;
use crate::filesystem;
use crate::filesystem::audio;
use crate::filesystem::extensions::SupportedExt;
use crate::parser;
use crate::state::{AppState, FetchLimitType};

pub mod archive;
pub mod bootstrap;
pub mod cache;
pub mod robots;

use robots::check_resource_rules;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);
type RateLimit = RateLimiter<String, DashMapStateStore<String>, QuantaClock>;

// TODO: Make this configurable by domain
const FETCH_DELAY_MS: i64 = 1000 * 60 * 60 * 24;

// TODO: Detect num of cpus & determine from there?
const AUDIO_TRANSCRIPTION_LIMIT: usize = 2;

#[derive(Debug, Error)]
pub enum CrawlError {
    #[error("crawl denied by rule {0}")]
    Denied(String),
    #[error("unable crawl document due to {0}")]
    FetchError(String),
    #[error("unable to parse document due to {0}")]
    ParseError(String),
    #[error("unable to read document due to {0}")]
    ReadError(#[from] std::io::Error),
    /// Document was not found.
    #[error("document not found")]
    NotFound,
    #[error("document was not modified since last check")]
    NotModified,
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
    /// Historically used as a short description of the page provided by the <meta>
    /// tag or summarized from the content. We generate previews now based on search
    /// terms + content.
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
    pub tags: Vec<TagPair>,
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

        let content = content.trim();
        let content = if content.is_empty() {
            None
        } else {
            Some(content.to_string())
        };

        Self {
            content_hash,
            content,
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
    pub client: Client,
    pub limiter: Arc<RateLimit>,
}

impl Default for Crawler {
    fn default() -> Self {
        Self::new(10)
    }
}

fn determine_canonical(original: &Url, extracted: Option<Url>) -> String {
    match extracted {
        None => {
            // Parse out the original path if the original URL was a web archive
            // link.
            if let Some(host) = original.host_str() {
                if host == "web.archive.org" {
                    let path = original.path().to_string();
                    // Split the web archive url & check to see if it's a valid URL
                    let splits = path.splitn(4, '/');
                    let canonical = splits.last().unwrap_or_default();
                    if let Ok(valid) = Url::parse(canonical) {
                        return valid.to_string();
                    }
                }
            }

            original.to_string()
        }
        Some(extracted) => {
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
            let extracted_dn =
                parse_domain_name(extracted_dn.expect("extracted_dn should not be None"));

            if origin_dn.is_err() || extracted_dn.is_err() {
                return original.to_string();
            }

            let origin_dn = origin_dn.expect("origin_dn invalid");
            let extracted_dn = extracted_dn.expect("extracted_dn invalid");

            // Special case for bootstrapper where we allow the canonical URL parsed
            // out of the HTML to override the original URL.
            if let Some(root) = origin_dn.root() {
                if root == "archive.org" || Some(root) == extracted_dn.root() {
                    return extracted.to_string();
                }
            }

            original.to_string()
        }
    }
}

impl Crawler {
    pub fn new(queries_per_second: u32) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            // TODO: Make configurable
            .connect_timeout(std::time::Duration::from_secs(3))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Unable to create reqwest client");

        let qps = if let Some(num) = NonZeroU32::new(queries_per_second) {
            num
        } else {
            nonzero!(2u32)
        };

        let quota = Quota::per_second(qps);

        Crawler {
            client,
            limiter: Arc::new(RateLimiter::<String, _, _>::keyed(quota)),
        }
    }

    /// Fetches and parses the content of a page.
    async fn crawl(&self, url: &Url, parse_results: bool) -> Result<CrawlResult, CrawlError> {
        match handle_crawl(&self.client, None, self.limiter.clone(), url).await {
            Ok(crawl) => {
                if parse_results {
                    let result = self.scrape_page(url, &crawl.headers, &crawl.content).await;
                    match result {
                        Some(crawl) => Ok(crawl),
                        None => Err(CrawlError::Unsupported(format!(
                            "Content Type unsupported {url:?}"
                        ))),
                    }
                } else {
                    Ok(CrawlResult {
                        url: crawl.url.clone(),
                        open_url: Some(crawl.url),
                        ..Default::default()
                    })
                }
            }
            Err(err) => Err(CrawlError::FetchError(err.to_string())),
        }
    }

    pub async fn scrape_page(
        &self,
        url: &Url,
        headers: &[(String, String)],
        raw_body: &str,
    ) -> Option<CrawlResult> {
        // Parse the html.
        log::debug!("Scraping page {:?}", url);
        let content_type = headers
            .iter()
            .find(|(header, _value)| header.eq("content-type"));
        if let Some((_header, value)) = content_type {
            if !is_html_content(value) {
                log::info!("Skipping content type {:?}", value);
                return None;
            }
        }
        let parse_result = html_to_text(url.as_ref(), raw_body);
        log::debug!("content hash: {:?}", parse_result.content_hash);

        let extracted = parse_result.canonical_url.and_then(|s| Url::parse(&s).ok());
        let canonical_url = determine_canonical(url, extracted);

        Some(CrawlResult {
            content_hash: Some(parse_result.content_hash),
            content: Some(parse_result.content),
            description: Some(parse_result.description),
            title: parse_result.title,
            url: canonical_url.clone(),
            open_url: Some(canonical_url),
            links: parse_result.links,
            ..Default::default()
        })
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
        let url = match Url::parse(&crawl.url) {
            Ok(url) => url,
            Err(_) => return Err(CrawlError::NotFound),
        };

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
            "file" => self.handle_file_fetch(state, &crawl, &url).await,
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
            Err(err) => Err(CrawlError::Unsupported(format!("{api_id}: {err}"))),
        }
    }

    async fn handle_file_fetch(
        &self,
        state: &AppState,
        task: &crawl_queue::Model,
        url: &Url,
    ) -> Result<CrawlResult, CrawlError> {
        // Attempt to convert from the URL to a file path
        let file_path = match url.to_file_path() {
            Ok(path) => path,
            Err(_) => return Err(CrawlError::NotFound),
        };

        let path = Path::new(&file_path);
        // Is this a file and does this exist?
        if !path.exists() {
            return Err(CrawlError::NotFound);
        }

        // Check when this was last modified against our last updated field
        let metadata = path.metadata()?;
        if let Ok(last_mod) = metadata.modified() {
            let last_modified: chrono::DateTime<Utc> = last_mod.into();
            if last_modified > task.updated_at {
                return Err(CrawlError::NotModified);
            }
        }

        let file_name = path
            .file_name()
            .and_then(|x| x.to_str())
            .map(|x| x.to_string())
            .expect("Unable to convert path file name to string");

        _process_path(state, path, file_name, url).await
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
                    path = format!("{path}?{query}");
                }

                let _ = fetch_history::upsert(db, domain, &path, result.content_hash.clone(), 200)
                    .await;

                Ok(result)
            }
        }
    }
}

async fn _process_file(
    state: &AppState,
    path: &Path,
    file_name: String,
    url: &Url,
) -> Result<CrawlResult, CrawlError> {
    // Attempt to read file
    let ext = path.extension();
    let mut content = None;

    if let Some(ext) = ext {
        match SupportedExt::from_ext(&ext.to_string_lossy()) {
            SupportedExt::Audio(_) => {
                if !state.fetch_limits.contains_key(&FetchLimitType::Audio) {
                    state.fetch_limits.insert(FetchLimitType::Audio, 0);
                }

                // Loop until audio transcription is finished
                while let Some(inflight) = state.fetch_limits.view(&FetchLimitType::Audio, |_, v| *v) {
                    if inflight >= AUDIO_TRANSCRIPTION_LIMIT {
                        log::debug!("`{}``: at transcription limit, waiting til finished!", file_name);
                        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    } else {
                        state.fetch_limits.alter(&FetchLimitType::Audio, |_, v| v + 1);
                        break;
                    }
                }

                log::debug!("starting transcription for `{}`", file_name);
                // Attempt to transcribe audio, assumes the model has been downloaded
                // and ready to go
                #[cfg(debug_assertions)]
                let model_path: PathBuf = "assets/models/whisper.base.en.bin".into();
                #[cfg(not(debug_assertions))]
                let model_path: PathBuf = _state.config.model_dir().join("whisper.base.en.bin");

                if !model_path.exists() {
                    log::warn!("whisper model not installed, skipping transcription");
                    content = None;
                } else {
                    match audio::transcibe_audio(path.to_path_buf(), model_path, 0) {
                        Ok(segments) => {
                            // Combine segments into one large string.
                            let combined = segments
                                .iter()
                                .map(|x| x.segment.to_string())
                                .collect::<Vec<String>>()
                                .join("");
                            content = Some(combined);
                        }
                        Err(err) => {
                            log::warn!(
                                "Skipping transcription: unable to transcribe: `{}`: {}",
                                path.display(),
                                err
                            );
                        }
                    }
                }

                // Say that we're finished
                state.fetch_limits.alter(
                    &FetchLimitType::Audio,
                    |_, v| v - 1
                );
            }
            SupportedExt::Document(_) => match parser::parse_file(ext, path) {
                Ok(parsed) => {
                    content = Some(parsed);
                }
                Err(err) => log::warn!("Unable to parse `{}`: {}", path.display(), err),
            },
            // todo: also parse symbols from code files.
            SupportedExt::Code(_) | SupportedExt::Text(_) => match std::fs::read_to_string(path) {
                Ok(x) => {
                    content = Some(x);
                }
                Err(err) => log::warn!("Unable to parse `{}`: {}", path.display(), err),
            },
            // Do nothing for these
            SupportedExt::NotSupported => {
                log::warn!("File `{:?}` unsupported returning empty content", path);
            }
        }
    }

    let content_hash = content.as_ref().map(|x| {
        let mut hasher = Sha256::new();
        hasher.update(x.as_bytes());
        hex::encode(&hasher.finalize()[..])
    });

    // TODO: Better description building for text files?
    let description = content.as_ref().map(|x| {
        x.split(' ')
            .take(DEFAULT_DESC_LENGTH)
            .collect::<Vec<&str>>()
            .join(" ")
    });

    let tags = filesystem::build_file_tags(path);
    Ok(CrawlResult {
        content_hash,
        content,
        // Does a file have a description? Pull the first part of the file
        description,
        title: Some(file_name),
        url: url.to_string(),
        open_url: Some(url.to_string()),
        links: Default::default(),
        tags,
    })
}

async fn _process_path(
    state: &AppState,
    path: &Path,
    file_name: String,
    url: &Url,
) -> Result<CrawlResult, CrawlError> {
    if path.is_file() {
        _process_file(state, path, file_name, url).await
    } else {
        Err(CrawlError::NotFound)
    }
}

fn _matches_ext(path: &Path, extension: &HashSet<String>) -> bool {
    let ext = &path
        .extension()
        .and_then(|x| x.to_str())
        .map(|x| x.to_string())
        .unwrap_or_default();
    extension.contains(ext)
}

fn is_html_content(content_type: &str) -> bool {
    content_type.contains("text/html") || content_type.contains("application/xhtml+xml")
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
        let crawler = Crawler::default();
        let url = Url::parse("https://oldschool.runescape.wiki").unwrap();
        let result = crawler.crawl(&url, true).await.expect("success");

        assert_eq!(result.title, Some("Old School RuneScape Wiki".to_string()));
        assert_eq!(result.url, "https://oldschool.runescape.wiki/".to_string());
        assert!(!result.links.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch() {
        let crawler = Crawler::default();

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
        let crawler = Crawler::default();
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
        let crawler = Crawler::default();
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
        let crawler = Crawler::default();

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
            normalize_href(url, "http://foo.com"),
            Some("https://foo.com/".into())
        );
        assert_eq!(
            normalize_href(url, "https://foo.com"),
            Some("https://foo.com/".into())
        );
        assert_eq!(
            normalize_href(url, "//foo.com"),
            Some("https://foo.com/".into())
        );
        assert_eq!(
            normalize_href(url, "/foo.html"),
            Some("https://example.com/foo.html".into())
        );
        assert_eq!(
            normalize_href(url, "/foo"),
            Some("https://example.com/foo".into())
        );
        assert_eq!(
            normalize_href(url, "foo.html"),
            Some("https://example.com/foo.html".into())
        );
    }

    #[test]
    fn test_determine_canonical() {
        // Test a correct override
        let a = Url::parse("https://commons.wikipedia.org").unwrap();
        let b = Url::parse("https://en.wikipedia.org").unwrap();

        let res = determine_canonical(&a, Some(b));
        assert_eq!(res, "https://en.wikipedia.org/");

        // Test a valid override from a different domain.
        let a = Url::parse("https://web.archive.org").unwrap();
        let b = Url::parse("https://en.wikipedia.org").unwrap();

        let res = determine_canonical(&a, Some(b));
        assert_eq!(res, "https://en.wikipedia.org/");

        // Test ignoring an invalid override
        let a = Url::parse("https://localhost:5000").unwrap();
        let b = Url::parse("https://en.wikipedia.org").unwrap();

        let res = determine_canonical(&a, Some(b));
        assert_eq!(res, "https://localhost:5000/");

        // Test ignoring an invalid override
        let a = Url::parse("https://en.wikipedia.org").unwrap();
        let b = Url::parse("https://spam.com").unwrap();

        let res = determine_canonical(&a, Some(b));
        assert_eq!(res, "https://en.wikipedia.org/");

        let a = Url::parse(
            "https://web.archive.org/web/20211209075429id_/https://docs.rs/test/0.0.1/lib.rs.html",
        )
        .unwrap();
        let res = determine_canonical(&a, None);
        assert_eq!(res, "https://docs.rs/test/0.0.1/lib.rs.html");
    }

    #[tokio::test]
    async fn test_file_fetch() {
        let crawler = Crawler::default();

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
        if let Err(error) = res {
            eprintln!("Error processing crawl {:?}", error);
            assert!(false);
        }
    }
}
