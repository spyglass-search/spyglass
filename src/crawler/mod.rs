use chrono::prelude::*;
use chrono::Duration;
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use std::fs;
use url::Url;

pub mod robots;

use crate::models::{CrawlQueue, DbPool, FetchHistory, ResourceRule};
use crate::scraper::html_to_text;
use crate::state::AppState;

use robots::parse;

// TODO: Make this configurable by domain
const FETCH_DELAY_MS: i64 = 100 * 60 * 60 * 24;

#[derive(Debug, Default, Clone)]
pub struct CrawlResult {
    pub content_hash: Option<String>,
    pub content: Option<String>,
    pub description: Option<String>,
    pub status: u16,
    pub title: Option<String>,
    pub url: Option<String>,
}

#[derive(Copy, Clone)]
pub struct Crawler;

impl Crawler {
    async fn crawl(url: &Url) -> CrawlResult {
        // Create a data directory for this domain
        let domain = url.host_str().unwrap();
        let domain_dir = AppState::crawl_dir().join(domain);
        if !domain_dir.exists() {
            fs::create_dir(&domain_dir).expect("Unable to create dir");
        }

        // Fetch & store page data.
        let res = reqwest::get(url.as_str()).await.unwrap();
        log::info!("Status: {}", res.status());
        let status = res.status();
        if status == StatusCode::OK {
            // TODO: Save headers
            // log::info!("Headers:\n{:?}", res.headers());
            let raw_body = res.text().await.unwrap();
            let file_path = domain_dir.join("raw.html");
            fs::write(file_path, &raw_body).expect("Unable to save html");

            // Parse the html.
            let parse_result = html_to_text(&raw_body);
            // Grab description from meta tags
            // TODO: Should we weight OpenGraph meta attrs more than others?
            let description = {
                if parse_result.meta.contains_key("description") {
                    Some(parse_result.meta.get("description").unwrap().to_string())
                } else if parse_result.meta.contains_key("og:description") {
                    Some(parse_result.meta.get("og:description").unwrap().to_string())
                } else {
                    None
                }
            };

            // Hash the body content, used to detect changes (eventually).
            let mut hasher = Sha256::new();
            hasher.update(&parse_result.content.as_bytes());
            let content_hash = Some(hex::encode(&hasher.finalize()[..]));

            log::info!("content hash: {:?}", content_hash);
            return CrawlResult {
                content_hash,
                content: Some(parse_result.content),
                description,
                status: status.as_u16(),
                title: parse_result.title,
                url: Some(url.to_string()),
            };
        }

        CrawlResult {
            status: status.as_u16(),
            ..Default::default()
        }
    }

    /// Checks whether we're allow to crawl this domain + path
    async fn is_crawl_allowed(db: &DbPool, domain: &str, path: &str) -> anyhow::Result<bool> {
        let mut rules = ResourceRule::find(db, domain).await?;
        log::info!("Found {} rules", rules.len());

        if rules.is_empty() {
            log::info!("No rules found for this domain, fetching robot.txt");
            let robots_url = format!("https://{}/robots.txt", domain);
            let res = reqwest::get(robots_url).await.unwrap();
            if res.status() == StatusCode::OK {
                let body = res.text().await.unwrap();

                rules = parse(domain, &body);
                log::info!("Found {} rules", rules.len());

                for rule in rules.iter() {
                    ResourceRule::insert_rule(db, rule).await?;
                }
            }
        }

        // Check path against rules, if we find any matches that disallow
        for res_rule in rules.iter() {
            if res_rule.rule.is_match(path) && !res_rule.allow_crawl {
                log::info!(
                    "Unable to crawl {} due to rule: {}:{}",
                    domain,
                    res_rule.rule,
                    res_rule.allow_crawl
                );
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Add url to the crawl queue
    pub async fn enqueue(db: &DbPool, url: &str) -> anyhow::Result<(), sqlx::Error> {
        CrawlQueue::insert(db, url, false).await
    }

    // TODO: Load web indexing as a plugin?
    pub async fn fetch(db: &DbPool, id: i64) -> anyhow::Result<Option<CrawlResult>, anyhow::Error> {
        let crawl = CrawlQueue::get(db, id).await?;

        log::info!("Fetching URL: {:?}", crawl.url);

        // Make sure cache directory exists for this domain
        let url = Url::parse(&crawl.url).unwrap();

        let domain = url.host_str().unwrap();
        let path = url.path();
        let url_base = format!("{}{}", domain, path);

        // Skip history check if we're trying to force this crawl.
        if !crawl.force_crawl {
            let history = FetchHistory::find(db, &url_base).await?;
            if let Some(history) = history {
                let since_last_fetch = Utc::now() - history.updated_at;
                if since_last_fetch < Duration::milliseconds(FETCH_DELAY_MS) {
                    log::info!("Recently fetched, skipping");
                    return Ok(None);
                }
            }
        }

        // Check for robots.txt of this domain
        if !Crawler::is_crawl_allowed(db, domain, url.path()).await? {
            CrawlQueue::mark_done(db, id).await?;
            return Ok(None);
        }

        // Crawl & save the data
        let result = Crawler::crawl(&url).await;
        log::info!(
            "crawl result: {:?} - {:?}\n{:?}",
            result.title,
            result.url,
            result.description,
        );

        // Update the fetch history & mark as done
        log::trace!("updating fetch history");
        FetchHistory::insert(db, &url_base, result.content_hash.clone(), result.status).await?;
        CrawlQueue::mark_done(db, id).await?;
        Ok(Some(result))
    }
}
