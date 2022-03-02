use sea_orm::prelude::*;
use sea_orm::{DatabaseConnection, Set};

use chrono::prelude::*;
use chrono::Duration;
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use std::fs;
use url::Url;

pub mod robots;
use robots::ParsedRule;

use crate::models::{crawl_queue, fetch_history, resource_rule};
use crate::scraper::html_to_text;
use crate::state::AppState;

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

        // Check path against rules, if we find any matches that disallow
        let rules_into: Vec<ParsedRule> = rules.iter().map(|x| x.to_owned().into()).collect();
        let filter_set = filter_set(&rules_into);
        if filter_set.is_match(path) {
            log::info!("Unable to crawl {} due to rule", domain);
            return Ok(false);
        }

        Ok(true)
    }

    /// Add url to the crawl queue
    pub async fn enqueue(db: &DatabaseConnection, url: &str) -> anyhow::Result<(), sea_orm::DbErr> {
        let new_task = crawl_queue::ActiveModel {
            url: Set(url.to_owned()),
            ..Default::default()
        };
        new_task.insert(db).await?;

        Ok(())
    }

    // TODO: Load web indexing as a plugin?
    pub async fn fetch(
        db: &DatabaseConnection,
        id: i64,
    ) -> anyhow::Result<Option<CrawlResult>, anyhow::Error> {
        let crawl = crawl_queue::Entity::find_by_id(id).one(db).await?.unwrap();

        log::info!("Fetching URL: {:?}", crawl.url);

        // Make sure cache directory exists for this domain
        let url = Url::parse(&crawl.url).unwrap();

        let domain = url.host_str().unwrap();
        let path = url.path();
        let url_base = format!("{}{}", domain, path);

        // Skip history check if we're trying to force this crawl.
        if !crawl.force_crawl {
            let history = fetch_history::Entity::find()
                .filter(fetch_history::Column::Url.eq(url_base.to_string()))
                .one(db)
                .await?;

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
            let mut updated: crawl_queue::ActiveModel = crawl.into();
            updated.status = Set(crawl_queue::CrawlStatus::Completed);
            updated.update(db).await?;

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
        fetch_history::upsert(db, &url_base, result.content_hash.clone(), result.status).await?;

        let mut updated: crawl_queue::ActiveModel = crawl.into();
        updated.status = Set(crawl_queue::CrawlStatus::Completed);
        updated.update(db).await?;

        Ok(Some(result))
    }
}

#[cfg(test)]
mod test {
    use sea_orm::prelude::*;
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
            Some("https://oldschool.runescape.wiki/".to_string())
        );
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
    async fn test_enqueue() {
        let db = setup_test_db().await;
        let url = "https://oldschool.runescape.wiki/";
        Crawler::enqueue(&db, url).await.unwrap();

        let crawl = crawl_queue::Entity::find()
            .filter(crawl_queue::Column::Url.eq(url.to_string()))
            .all(&db)
            .await
            .unwrap();

        assert_eq!(crawl.len(), 1);
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
            Some("https://oldschool.runescape.wiki/".to_string())
        );
    }
}
