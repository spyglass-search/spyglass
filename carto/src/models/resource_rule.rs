#![allow(dead_code)]

use chrono::prelude::*;
use regex::Regex;
use rusqlite::{params, Connection};

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
                no_index: row.get(3)?,
                allow_crawl: row.get(4)?,
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
            params![domain, rule, no_index, allow_crawl,],
        )?;

        Ok(())
    }

    pub fn insert_rule(db: &Connection, rule: &ResourceRule) -> Result<(), rusqlite::Error> {
        db.execute(
            "INSERT INTO resource_rules (domain, rule, no_index, allow_crawl)
            VALUES (?1, ?2, ?3, ?4)",
            params![
                &rule.domain,
                &rule.rule.to_string(),
                rule.no_index,
                rule.allow_crawl,
            ],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::models::ResourceRule;
    use rusqlite::Connection;

    #[test]
    fn test_init() {
        let db = Connection::open_in_memory().unwrap();
        ResourceRule::init_table(&db);
    }

    #[test]
    fn test_insert() {
        let db = Connection::open_in_memory().unwrap();
        ResourceRule::init_table(&db);

        let res = ResourceRule::insert(&db, "oldschool.runescape.wiki", "/", false, true);
        assert!(res.is_ok());

        let rules =
            ResourceRule::find(&db, "oldschool.runescape.wiki").expect("Unable to find rules");
        assert_eq!(rules.len(), 1);
    }
}
