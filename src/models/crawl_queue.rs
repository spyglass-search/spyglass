use chrono::prelude::*;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::models::DbPool;

pub struct CrawlQueue {
    pub id: Option<u64>,
    /// URL to crawl
    pub url: String,
    /// When this was first added to the crawl queue
    pub created_at: DateTime<Utc>,
}

impl CrawlQueue {
    pub async fn init_table(db: &DbPool) -> anyhow::Result<(), sqlx::Error> {
        let mut conn = db.acquire().await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS crawl_queue (
                id INTEGER PRIMARY KEY,
                url TEXT UNIQUE,
                created_at DATETIME default CURRENT_TIMESTAMP
            )",
        )
        .execute(&mut conn)
        .await?;

        Ok(())
    }

    pub async fn insert(db: &DbPool, url: &str) -> anyhow::Result<(), sqlx::Error> {
        let mut conn = db.acquire().await?;

        sqlx::query("INSERT OR IGNORE INTO crawl_queue (url) VALUES (?)")
            .bind(url)
            .execute(&mut conn)
            .await?;

        Ok(())
    }

    pub async fn next(db: &DbPool) -> anyhow::Result<Option<String>, sqlx::Error> {
        let mut conn = db.begin().await?;
        let row: Option<SqliteRow> =
            sqlx::query("SELECT id, url FROM crawl_queue ORDER BY created_at ASC LIMIT 1")
                .fetch_optional(&mut conn)
                .await?;

        if let Some(row) = row {
            let id: i64 = row.get(0);
            let url: String = row.get(1);

            sqlx::query("DELETE FROM crawl_queue WHERE id = ?")
                .bind(id)
                .execute(&mut conn)
                .await?;

            conn.commit().await?;
            return Ok(Some(url));
        }

        conn.commit().await?;
        Ok(None)
    }
}
