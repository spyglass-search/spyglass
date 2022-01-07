#![allow(dead_code)]

use chrono::prelude::*;
use regex::Regex;
use rusqlite::{params, Connection};
use url::Url;

#[derive(Debug)]
pub struct Place {
    pub id: i32,
    pub url: Url,
}

#[derive(Debug)]
pub struct ResourceRule {
    /// Optional to represent rows that not been inserted yet
    pub id: Option<u64>,
    pub domain: String,
    pub rule: Regex,
    pub no_index: bool,
    pub allow_crawl: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ResourceRule {
    pub fn new(domain: &str, rule: &Regex, no_index: bool, allow_crawl: bool) -> Self {
        ResourceRule {
            id: None,
            domain: domain.to_string(),
            rule: rule.clone(),
            no_index,
            allow_crawl,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    pub fn init_table(db: &Connection) {
        db.execute(
            "CREATE TABLE IF NOT EXISTS resource_rules (
                id INTEGER PRIMARY KEY,
                domain TEXT,
                rule TEXT,
                no_index BOOLEAN,
                allow_crawl BOOLEAN,
                created_at DATETIME default CURRENT_TIMESTAMP,
                updated_at DATETIME default CURRENT_TIMESTAMP,
                UNIQUE (domain, rule) ON CONFLICT IGNORE
            )",
            [],
        )
        .expect("Unable to intitialize `robots_txt` table");
    }

    pub fn find(db: &Connection, domain: &str) -> Result<Vec<ResourceRule>, rusqlite::Error> {
        let mut stmt = db.prepare(
            "SELECT
                id,
                domain,
                rule,
                no_index,
                allow_crawl,
                created_at,
                updated_at
            FROM resource_rules
            WHERE domain = ?",
        )?;

        if !stmt.exists(params![domain])? {
            return Ok(Vec::new());
        }

        let mapped_rows = stmt.query_map(params![domain], |row| {
            // Rules are stored as a JSON blob
            let rule_str: String = row.get(2)?;
            Ok(ResourceRule {
                id: Some(row.get(0)?),
                domain: row.get(1)?,
                rule: Regex::new(&rule_str).unwrap(),
                no_index: row.get::<usize, String>(3)? == "true",
                allow_crawl: row.get::<usize, String>(4)? == "true",
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;

        let mut res = Vec::new();
        for row in mapped_rows {
            res.push(row?);
        }

        Ok(res)
    }

    pub fn insert(
        db: &Connection,
        domain: &str,
        rule: &str,
        no_index: bool,
        allow_crawl: bool,
    ) -> Result<(), rusqlite::Error> {
        db.execute(
            "INSERT INTO resource_rules (domain, rule, no_index, allow_crawl)
            VALUES (?1, ?2, ?3, ?4)",
            [
                domain,
                rule,
                &no_index.to_string(),
                &allow_crawl.to_string(),
            ],
        )?;

        Ok(())
    }

    pub fn insert_rule(db: &Connection, rule: &ResourceRule) -> Result<(), rusqlite::Error> {
        db.execute(
            "INSERT INTO resource_rules (domain, rule, no_index, allow_crawl)
            VALUES (?1, ?2, ?3, ?4)",
            [
                &rule.domain,
                &rule.rule.to_string(),
                &rule.no_index.to_string(),
                &rule.allow_crawl.to_string(),
            ],
        )?;

        Ok(())
    }
}

/// When a URL was last fetched. Also used as a queue for the indexer to determine
/// what paths to index next.
#[derive(Debug)]
pub struct FetchHistory {
    /// Arbitrary id for this.
    id: u64,
    /// URL fetched.
    url: Url,
    /// Hash used to check for changes.
    hash: u64,
    /// HTTP status when last fetching this page.
    status: u8,
    /// Ignore this URL in the future.
    no_index: bool,
    /// When this was first added to our fetch history
    created_at: DateTime<Utc>,
    /// When this URL was last fetched.
    updated_at: DateTime<Utc>,
}

impl FetchHistory {
    pub fn init_table(db: &Connection) {
        db.execute(
            "CREATE TABLE IF NOT EXISTS fetch_history (
                id INTEGER PRIMARY KEY,
                url TEXT UNIQUE,
                hash TEXT,
                status INTEGER,
                no_index BOOLEAN,
                created_at DATETIME,
                updated_at DATETIME
            )",
            [],
        )
        .expect("Unable to init `fetch_history` table");
    }
}
