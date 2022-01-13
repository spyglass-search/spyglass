#![allow(dead_code)]
use chrono::prelude::*;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::models::DbPool;

/// When a URL was last fetched. Also used as a queue for the indexer to determine
/// what paths to index next.
#[derive(Debug)]
pub struct FetchHistory {
    /// Arbitrary id for this.
    pub id: Option<i64>,
    /// URL fetched.
    pub url: String,
    /// Hash used to check for changes.
    pub hash: Option<String>,
    /// HTTP status when last fetching this page.
    pub status: u16,
    /// Ignore this URL in the future.
    pub no_index: bool,
    /// When this was first added to our fetch history
    pub created_at: DateTime<Utc>,
    /// When this URL was last fetched.
    pub updated_at: DateTime<Utc>,
}

impl FetchHistory {
    pub async fn init_table(db: &DbPool) -> anyhow::Result<(), sqlx::Error> {
        let mut conn = db.acquire().await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS fetch_history (
                id INTEGER PRIMARY KEY,
                url TEXT UNIQUE,
                hash TEXT,
                status INTEGER,
                no_index BOOLEAN,
                created_at DATETIME default CURRENT_TIMESTAMP,
                updated_at DATETIME default CURRENT_TIMESTAMP
            )",
        )
        .execute(&mut conn)
        .await?;

        Ok(())
    }

    pub async fn find(db: &DbPool, url: &str) -> anyhow::Result<Option<FetchHistory>, sqlx::Error> {
        let mut conn = db.acquire().await?;

        let row: Option<SqliteRow> = sqlx::query(
            "SELECT
                id,
                url,
                hash,
                status,
                no_index,
                created_at,
                updated_at
                FROM fetch_history WHERE url = ?",
        )
        .bind(url)
        .fetch_optional(&mut conn)
        .await?;

        if let Some(row) = row {
            return Ok(Some(FetchHistory {
                id: row.get::<Option<i64>, _>(0),
                url: row.get::<String, _>(1),
                hash: row.get::<Option<String>, _>(2),
                status: row.get::<u16, _>(3),
                no_index: row.get(4),
                created_at: row.get(5),
                updated_at: row.get(6),
            }));
        }

        Ok(None)
    }

    pub async fn insert(
        db: &DbPool,
        url: &str,
        hash: Option<String>,
        status: u16,
    ) -> anyhow::Result<(), sqlx::Error> {
        let mut conn = db.acquire().await?;

        sqlx::query(
            "INSERT INTO fetch_history (url, hash, status, no_index)
                VALUES (?, ?, ?, ?)
                ON CONFLICT(url) DO UPDATE SET
                    updated_at = CURRENT_TIMESTAMP,
                    hash = ?
                ",
        )
        .bind(url)
        .bind(&hash)
        .bind(status)
        .bind(false)
        .bind(&hash)
        .execute(&mut conn)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::config::Config;
    use crate::models::{create_connection, FetchHistory};
    use std::path::Path;

    #[tokio::test]
    async fn test_insert() {
        let config = Config {
            data_dir: Path::new("/tmp").to_path_buf(),
            prefs_dir: Path::new("/tmp").to_path_buf(),
        };

        let db = create_connection(&config).await.unwrap();
        FetchHistory::init_table(&db).await.unwrap();

        let hash = "this is a hash".to_string();
        FetchHistory::insert(&db, "oldschool.runescape.wiki/", Some(hash.clone()), 200)
            .await
            .unwrap();

        let url = "oldschool.runescape.wiki/";
        let history = FetchHistory::find(&db, url).await.unwrap().unwrap();
        assert_eq!(history.url, url);
        assert_eq!(history.hash.unwrap(), hash);
    }
}
