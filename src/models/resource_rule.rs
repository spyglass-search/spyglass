#![allow(dead_code)]

use chrono::prelude::*;
use regex::Regex;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::models::DbPool;

#[derive(Debug)]
pub struct ResourceRule {
    /// Optional to represent rows that not been inserted yet
    pub id: Option<i64>,
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

    pub async fn init_table(db: &DbPool) -> anyhow::Result<(), sqlx::Error> {
        let mut conn = db.acquire().await?;

        sqlx::query(
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
        )
        .execute(&mut conn)
        .await?;

        Ok(())
    }

    pub async fn find(db: &DbPool, domain: &str) -> anyhow::Result<Vec<ResourceRule>, sqlx::Error> {
        let mut conn = db.acquire().await?;

        let rows = sqlx::query(
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
        )
        .bind(domain)
        .try_map(|row: SqliteRow| {
            let rule_str = row.get(2);
            let rule = ResourceRule {
                id: row.get::<Option<i64>, _>(0),
                domain: row.get(1),
                rule: Regex::new(rule_str).unwrap(),
                no_index: row.get(3),
                allow_crawl: row.get(4),
                created_at: row.get(5),
                updated_at: row.get(6),
            };

            Ok(rule)
        })
        .fetch_all(&mut conn)
        .await?;

        Ok(rows)
    }

    pub async fn insert(
        db: &DbPool,
        domain: &str,
        rule: &str,
        no_index: bool,
        allow_crawl: bool,
    ) -> Result<(), sqlx::Error> {
        let mut conn = db.acquire().await?;

        sqlx::query(
            "INSERT INTO resource_rules (domain, rule, no_index, allow_crawl)
            VALUES (?, ?, ?, ?)",
        )
        .bind(domain)
        .bind(rule)
        .bind(no_index)
        .bind(allow_crawl)
        .execute(&mut conn)
        .await?;

        Ok(())
    }

    pub async fn insert_rule(db: &DbPool, rule: &ResourceRule) -> Result<(), sqlx::Error> {
        let mut conn = db.acquire().await?;

        sqlx::query(
            "INSERT INTO resource_rules (domain, rule, no_index, allow_crawl)
            VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&rule.domain)
        .bind(&rule.rule.to_string())
        .bind(rule.no_index)
        .bind(rule.allow_crawl)
        .execute(&mut conn)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::config::Config;
    use crate::models::{create_connection, ResourceRule};
    use std::path::Path;

    #[tokio::test]
    async fn test_insert() -> anyhow::Result<(), sqlx::Error> {
        let config = Config {
            data_dir: Path::new("/tmp").to_path_buf(),
            prefs_dir: Path::new("/tmp").to_path_buf(),
        };

        let db = create_connection(&config).await.unwrap();
        ResourceRule::init_table(&db).await?;

        let res = ResourceRule::insert(&db, "oldschool.runescape.wiki", "/", false, true).await;
        assert!(res.is_ok());

        let rules = ResourceRule::find(&db, "oldschool.runescape.wiki")
            .await
            .expect("Unable to find rules");
        assert_eq!(rules.len(), 1);

        Ok(())
    }
}
