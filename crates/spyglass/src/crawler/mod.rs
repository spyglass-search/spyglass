use std::collections::HashSet;

use chrono::prelude::*;
use chrono::Duration;
use reqwest::{Client, StatusCode};
use sea_orm::prelude::*;
use sea_orm::{DatabaseConnection, Set};
use sha2::{Digest, Sha256};
use url::Url;

pub mod robots;
use robots::ParsedRule;

use crate::models::{crawl_queue, fetch_history, resource_rule};
use crate::scraper::html_to_text;

use robots::{filter_set, parse};

// TODO: Make this configurable by domain
const FETCH_DELAY_MS: i64 = 100 * 60 * 60 * 24;
static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[derive(Debug, Default, Clone)]
pub struct CrawlResult {
    pub content_hash: Option<String>,
    /// Text content from page after stripping HTML tags & any semantically
    /// unimportant sections (header/footer/etc.)
    pub content: Option<String>,
    /// A short description of the page provided by the <meta> tag or summarized
    /// from the content.
    pub description: Option<String>,
    pub status: u16,
    pub title: Option<String>,
    pub url: String,
    /// Links found in the page to add to the queue.
    pub links: HashSet<String>,
    /// Raw HTML data.
    pub raw: Option<String>,
}

impl CrawlResult {
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status <= 299
    }
}

fn _normalize_href(url: &Url, href: &str) -> Option<String> {
    // Force HTTPS, crawler will fallback to HTTP if necessary.
    if href.starts_with("//") {
        // schema relative url
        if let Ok(url) = Url::parse(&format!("{}:{}", "https", href)) {
            return Some(url.to_string());
        }
    } else if href.starts_with("http://") || href.starts_with("https://") {
        // Force HTTPS, crawler will fallback to HTTP if necessary.
        if let Ok(url) = Url::parse(href) {
            let mut url = url;
            url.set_scheme("https").unwrap();
            return Some(url.to_string());
        }
    } else {
        // origin or directory relative url
        if let Ok(url) = url.join(href) {
            return Some(url.to_string());
        }
    }

    log::debug!("Unable to normalize href: {} - {}", url.to_string(), href);
    None
}

#[derive(Clone)]
pub struct Crawler {
    pub client: Client,
}

impl Default for Crawler {
    fn default() -> Self {
        Self::new()
    }
}

impl Crawler {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(APP_USER_AGENT)
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Unable to create reqwest client");

        Crawler { client }
    }
    /// Fetches and parses the content of a page.
    async fn crawl(&self, url: &Url) -> CrawlResult {
        // Force HTTPs
        let mut url = url.clone();
        url.set_scheme("https").unwrap();

        // Fetch & store page data.
        let res = self.client.get(url.as_str()).send().await;
        if res.is_err() {
            // Unable to connect to host
            return CrawlResult {
                // Service unavilable
                status: 503_u16,
                url: url.to_string(),
                ..Default::default()
            };
        }

        let res = res.unwrap();
        let status = res.status().as_u16();
        if status == StatusCode::OK {
            let raw_body = res.text().await.unwrap();
            let mut scrape_result = self.scrape_page(&url, &raw_body).await;
            scrape_result.status = status;
            return scrape_result;
        }

        CrawlResult {
            status,
            url: url.to_string(),
            ..Default::default()
        }
    }

    pub async fn scrape_page(&self, url: &Url, raw_body: &str) -> CrawlResult {
        // Parse the html.
        let parse_result = html_to_text(raw_body);

        // Hash the body content, used to detect changes (eventually).
        let mut hasher = Sha256::new();
        hasher.update(&parse_result.content.as_bytes());
        let content_hash = Some(hex::encode(&hasher.finalize()[..]));

        // Normalize links from scrape result. If the links start with "/" they
        // should be appended to the current URL.
        let normalized_links = parse_result
            .links
            .iter()
            .filter_map(|link| _normalize_href(url, link))
            .collect();

        log::info!("content hash: {:?}", content_hash);
        CrawlResult {
            content_hash,
            content: Some(parse_result.content),
            description: Some(parse_result.description),
            status: 200,
            title: parse_result.title,
            url: url.to_string(),
            links: normalized_links,
            raw: Some(raw_body.to_string()),
        }
    }

    /// Checks whether we're allow to crawl this domain + path
    async fn is_crawl_allowed(
        &self,
        db: &DatabaseConnection,
        domain: &str,
        path: &str,
        full_url: &str,
    ) -> anyhow::Result<bool> {
        let rules = resource_rule::Entity::find()
            .filter(resource_rule::Column::Domain.eq(domain))
            .all(db)
            .await?;

        if rules.is_empty() {
            log::info!("No rules found for this domain, fetching robot.txt");

            let robots_url = format!("https://{}/robots.txt", domain);
            let res = self.client.get(robots_url).send().await;
            match res {
                Err(err) => log::error!("Unable to check robots.txt {}", err.to_string()),
                Ok(res) => {
                    if res.status() == StatusCode::OK {
                        let body = res.text().await.unwrap();

                        let parsed_rules = parse(domain, &body);
                        for rule in parsed_rules.iter() {
                            let new_rule = resource_rule::ActiveModel {
                                domain: Set(rule.domain.to_owned()),
                                rule: Set(rule.regex.to_owned()),
                                no_index: Set(false),
                                allow_crawl: Set(rule.allow_crawl),
                                ..Default::default()
                            };
                            new_rule.insert(db).await?;
                        }
                    }
                }
            }
        }

        // Check path against rules, if we find any matches that disallow, skip it
        let rules_into: Vec<ParsedRule> = rules.iter().map(|x| x.to_owned().into()).collect();

        let allow_filter = filter_set(&rules_into, true);
        let disallow_filter = filter_set(&rules_into, false);
        if !allow_filter.is_match(path) && disallow_filter.is_match(path) {
            log::info!("Unable to crawl {}|{} due to rule", domain, path);
            return Ok(false);
        }

        // Check the content-type of the URL, only crawl HTML pages for now
        let res = self.client.head(full_url).send().await;

        match res {
            Err(err) => {
                log::info!("Unable to check content-type: {}", err.to_string());
                return Ok(false);
            }
            Ok(res) => {
                let headers = res.headers();
                if !headers.contains_key(http::header::CONTENT_TYPE) {
                    return Ok(false);
                } else {
                    let value = headers.get(http::header::CONTENT_TYPE).unwrap();
                    let value = value.to_str().unwrap();
                    if !value.to_string().contains(&"text/html") {
                        log::info!("Unable to crawl: content-type =/= text/html");
                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }

    // TODO: Load web indexing as a plugin?
    /// Attempts to crawl a job from the crawl_queue specific by <id>
    /// * Checks whether we can crawl using any saved rules or looking at the robots.txt
    /// * Fetches & parses the page
    pub async fn fetch_by_job(
        &self,
        db: &DatabaseConnection,
        id: i64,
    ) -> anyhow::Result<Option<CrawlResult>, anyhow::Error> {
        let crawl = crawl_queue::Entity::find_by_id(id).one(db).await?.unwrap();

        log::info!("Fetching URL: {:?}", crawl.url);
        let url = Url::parse(&crawl.url).unwrap();

        // Break apart domain + path of the URL
        let domain = url.host_str().unwrap();
        let path = url.path();

        // Skip history check if we're trying to force this crawl.
        if !crawl.force_crawl {
            if let Some(history) = fetch_history::find_by_url(db, &url).await? {
                let since_last_fetch = Utc::now() - history.updated_at;
                if since_last_fetch < Duration::milliseconds(FETCH_DELAY_MS) {
                    log::info!("Recently fetched, skipping");
                    return Ok(None);
                }
            }
        }

        // Check for robots.txt of this domain
        if !self
            .is_crawl_allowed(db, domain, url.path(), url.as_str())
            .await?
        {
            return Ok(None);
        }

        // Crawl & save the data
        let result = self.crawl(&url).await;
        log::info!(
            "crawl result: {:?} - {:?}\n{:?}",
            result.title,
            result.url,
            result.description,
        );

        // Update fetch history
        fetch_history::upsert(db, domain, path, result.content_hash.clone(), result.status).await?;

        Ok(Some(result))
    }
}

#[cfg(test)]
mod test {
    use sea_orm::{ActiveModelTrait, Set};

    use crate::crawler::{Crawler, _normalize_href};
    use crate::models::{crawl_queue, resource_rule};
    use crate::test::setup_test_db;

    use url::Url;

    #[tokio::test]
    async fn test_crawl() {
        let crawler = Crawler::new();
        let url = Url::parse("https://oldschool.runescape.wiki").unwrap();
        let result = crawler.crawl(&url).await;

        assert_eq!(result.title, Some("Old School RuneScape Wiki".to_string()));
        assert_eq!(result.url, "https://oldschool.runescape.wiki/".to_string());

        // All links should start w/ http
        for link in result.links {
            assert!(link.starts_with("https://"))
        }
    }

    #[tokio::test]
    async fn test_is_crawl_allowed() {
        let crawler = Crawler::new();
        let db = setup_test_db().await;

        let url = "https://oldschool.runescape.wiki/";
        let domain = "oldschool.runescape.wiki";
        let rule = "/";

        // Add some fake rules
        let allow = resource_rule::ActiveModel {
            domain: Set(domain.to_owned()),
            rule: Set(rule.to_owned()),
            no_index: Set(false),
            allow_crawl: Set(true),
            ..Default::default()
        };
        allow
            .insert(&db)
            .await
            .expect("Unable to insert allow rule");

        let res = crawler
            .is_crawl_allowed(&db, domain, rule, url)
            .await
            .unwrap();
        assert_eq!(res, true);
    }

    #[tokio::test]
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

        let crawl_result = crawler.fetch_by_job(&db, model.id).await.unwrap();
        assert!(crawl_result.is_some());

        let result = crawl_result.unwrap();
        assert_eq!(result.title, Some("Old School RuneScape Wiki".to_string()));
        assert_eq!(result.url, "https://oldschool.runescape.wiki/".to_string());
    }

    #[test]
    fn test_normalize_href() {
        let url = Url::parse("https://example.com").unwrap();

        assert_eq!(
            _normalize_href(&url, "http://foo.com"),
            Some("https://foo.com/".into())
        );
        assert_eq!(
            _normalize_href(&url, "https://foo.com"),
            Some("https://foo.com/".into())
        );
        assert_eq!(
            _normalize_href(&url, "//foo.com"),
            Some("https://foo.com/".into())
        );
        assert_eq!(
            _normalize_href(&url, "/foo.html"),
            Some("https://example.com/foo.html".into())
        );
        assert_eq!(
            _normalize_href(&url, "/foo"),
            Some("https://example.com/foo".into())
        );
        assert_eq!(
            _normalize_href(&url, "foo.html"),
            Some("https://example.com/foo.html".into())
        );
    }
}
