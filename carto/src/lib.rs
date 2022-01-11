use chrono::prelude::*;
use chrono::Duration;
use directories::ProjectDirs;
use reqwest::StatusCode;
use rusqlite::{Connection, OpenFlags, Result};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
use url::Url;

pub mod models;
pub mod robots;
use models::{FetchHistory, Place, ResourceRule};
use robots::parse;

// TODO: Make this configurable by domain
const FETCH_DELAY_MS: i64 = 100 * 60 * 60 * 24;

struct CrawlResult {
    status: u16,
    content_hash: Option<String>,
}

pub struct Carto {
    db: Connection,
    data_dir: PathBuf,
}

impl Carto {
    pub fn init_db(&self) {
        // Initialize robots table
        ResourceRule::init_table(&self.db);
        // Initialize fetch history table
        FetchHistory::init_table(&self.db);
    }

    pub fn init() -> Self {
        let proj_dirs = ProjectDirs::from("com", "athlabs", "carto").unwrap();
        let data_dir = proj_dirs.data_dir().join("crawls");

        fs::create_dir_all(&data_dir).expect("Unable to create crawl folder");

        let db_path = proj_dirs.data_dir().join("db.sqlite");
        dbg!(&db_path);
        let db = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .unwrap();

        let carto = Carto { db, data_dir };
        carto.init_db();

        carto
    }

    async fn crawl(&self, url: &Url) -> CrawlResult {
        // Create a data directory for this domain
        let domain = url.host_str().unwrap();
        let domain_dir = self.data_dir.join(domain);
        if !domain_dir.exists() {
            fs::create_dir(&domain_dir).expect("Unable to create dir");
        }

        // Fetch & store page data.
        log::info!("Fetching page: {}", url.as_str());
        let res = reqwest::get(url.as_str()).await.unwrap();
        log::info!("Status: {}", res.status());
        let status = res.status();
        if status == StatusCode::OK {
            // TODO: Save headers
            // log::info!("Headers:\n{:?}", res.headers());
            let body = res.text().await.unwrap();
            let file_path = domain_dir.join("raw.html");
            fs::write(file_path, &body).expect("Unable to save html");

            // Hash the body contents
            let mut hasher = Sha256::new();
            hasher.update(&body.as_bytes());
            let content_hash = Some(hex::encode(&hasher.finalize()[..]));
            return CrawlResult {
                status: status.as_u16(),
                content_hash,
            };
        }

        CrawlResult {
            status: status.as_u16(),
            content_hash: None,
        }
    }

    /// Checks whether we're allow to crawl this domain + path
    async fn is_crawl_allowed(&self, domain: &str, path: &str) -> Result<bool, rusqlite::Error> {
        let mut rules = ResourceRule::find(&self.db, domain)?;
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
                    ResourceRule::insert_rule(&self.db, rule)?;
                }
            }
        }

        // Check path against rules, if we find any matches that disallow
        for res_rule in rules.iter() {
            if res_rule.rule.is_match(path) && !res_rule.allow_crawl {
                return Ok(false);
            }
        }

        Ok(true)
    }

    // TODO: Load web indexing as a plugin?
    pub async fn fetch(&self, place: &Place) -> Result<(), rusqlite::Error> {
        // Make sure cache directory exists for this domain
        let url = &place.url;

        let domain = url.host_str().unwrap();
        let path = url.path();
        let url_base = format!("{}{}", domain, path);

        let history = FetchHistory::find(&self.db, &url_base)?;
        if let Some(history) = history {
            let since_last_fetch = Utc::now() - history.updated_at;
            if since_last_fetch < Duration::milliseconds(FETCH_DELAY_MS) {
                log::info!("Recently fetched, skipping");
                return Ok(());
            }
        }

        // Check for robots.txt of this domain
        if !self.is_crawl_allowed(domain, url.path()).await? {
            return Ok(());
        }

        // Crawl & save the data
        let result = self.crawl(url).await;
        // Update the fetch history for this path
        log::info!("Updated fetch history");
        FetchHistory::insert(&self.db, &url_base, result.content_hash, result.status)?;

        Ok(())
    }
}
