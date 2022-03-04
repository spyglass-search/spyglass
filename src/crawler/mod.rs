use std::collections::HashSet;

use chrono::prelude::*;
use chrono::Duration;
use reqwest::StatusCode;
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

#[derive(Debug, Default, Clone)]
pub struct CrawlResult {
    pub content_hash: Option<String>,
    pub content: Option<String>,
    pub description: Option<String>,
    pub status: u16,
    pub title: Option<String>,
    pub url: String,
    /// Links found in the page to add to the queue.
    pub links: HashSet<String>,
    /// Raw HTML data.
    pub raw: Option<String>,
}

#[derive(Copy, Clone)]
pub struct Crawler;

impl Crawler {
    /// Fetches and parses the content of a page.
    async fn crawl(url: &Url) -> CrawlResult {
        // Fetch & store page data.
        let res = reqwest::get(url.as_str()).await.unwrap();
        log::info!("Status: {}", res.status());
        let status = res.status();
        if status == StatusCode::OK {
            // TODO: Save headers
            // log::info!("Headers:\n{:?}", res.headers());
            let raw_body = res.text().await.unwrap();
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

            // Normalize links from scrape result. If the links start with "/" they should
            // be appended to the current URL.
            let normalized_links = parse_result.links
                .iter()
                .map(|link| {
                    if link.starts_with('/') {
                        url.join(link).unwrap().as_str().to_string()
                    } else {
                        link.to_owned()
                    }
                })
                .collect();


            log::info!("content hash: {:?}", content_hash);
            return CrawlResult {
                content_hash,
                content: Some(parse_result.content),
                description,
                status: status.as_u16(),
                title: parse_result.title,
                url: url.to_string(),
                links: normalized_links,
                raw: Some(raw_body),
            };
        }

        CrawlResult {
            status: status.as_u16(),
            url: url.to_string(),
            ..Default::default()
        }
    }

    /// Checks whether we're allow to crawl this domain + path
    async fn is_crawl_allowed(
        db: &DatabaseConnection,
        domain: &str,
        path: &str,
    ) -> anyhow::Result<bool> {
        let rules = resource_rule::Entity::find()
            .filter(resource_rule::Column::Domain.eq(domain))
            .all(db)
            .await?;

        log::info!("Found {} rules", rules.len());

        if rules.is_empty() {
            log::info!("No rules found for this domain, fetching robot.txt");
            let robots_url = format!("https://{}/robots.txt", domain);
            let res = reqwest::get(robots_url).await.unwrap();
            if res.status() == StatusCode::OK {
                let body = res.text().await.unwrap();

                let parsed_rules = parse(domain, &body);
                log::info!("Found {} rules", rules.len());

                for rule in parsed_rules.iter() {
                    let new_rule = resource_rule::ActiveModel {
                        domain: Set(rule.domain.to_owned()),
                        rule: Set(rule.regex.to_owned()),
                        no_index: Set(rule.no_index),
                        allow_crawl: Set(rule.allow_crawl),
                        ..Default::default()
                    };
                    new_rule.insert(db).await?;
                }
            }
        }

        // Check path against rules, if we find any matches that disallow, skip it
        let rules_into: Vec<ParsedRule> = rules.iter().map(|x| x.to_owned().into()).collect();
        let filter_set = filter_set(&rules_into);
        if filter_set.is_match(path) {
            log::info!("Unable to crawl {}|{} due to rule", domain, path);
            return Ok(false);
        }

        Ok(true)
    }

    // TODO: Load web indexing as a plugin?
    /// Attempts to crawl a job from the crawl_queue specific by <id>
    /// * Checks whether we can crawl using any saved rules or looking at the robots.txt
    /// * Fetches & parses the page
    pub async fn fetch(
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
        if !Crawler::is_crawl_allowed(db, domain, url.path()).await? {
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

        // Update fetch history
        fetch_history::upsert(db, domain, path, result.content_hash.clone(), result.status).await?;

        Ok(Some(result))
    }
}

#[cfg(test)]
mod test {
    use sea_orm::{ActiveModelTrait, Set};

    use crate::crawler::Crawler;
    use crate::models::{crawl_queue, resource_rule};
    use crate::test::setup_test_db;

    use url::Url;

    #[tokio::test]
    async fn test_crawl() {
        let url = Url::parse("https://oldschool.runescape.wiki").unwrap();
        let result = Crawler::crawl(&url).await;

        assert_eq!(result.title, Some("Old School RuneScape Wiki".to_string()));
        assert_eq!(
            result.url,
            "https://oldschool.runescape.wiki/".to_string()
        );

        // All links should start w/ http
        for link in result.links {
            assert!(link.starts_with("https://"))
        }
    }

    #[tokio::test]
    async fn test_is_crawl_allowed() {
        let db = setup_test_db().await;

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

        let res = Crawler::is_crawl_allowed(&db, domain, rule).await.unwrap();
        assert_eq!(res, true);
    }

    #[tokio::test]
    async fn test_fetch() {
        let db = setup_test_db().await;
        let url = "https://oldschool.runescape.wiki/";
        let query = crawl_queue::ActiveModel {
            url: Set(url.to_owned()),
            ..Default::default()
        };
        let model = query.insert(&db).await.unwrap();

        let crawl_result = Crawler::fetch(&db, model.id).await.unwrap();
        assert!(crawl_result.is_some());

        let result = crawl_result.unwrap();
        assert_eq!(result.title, Some("Old School RuneScape Wiki".to_string()));
        assert_eq!(
            result.url,
            "https://oldschool.runescape.wiki/".to_string()
        );
    }
}
