use chrono::prelude::*;
use serde::Serialize;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::models::DbPool;

#[derive(Debug, Serialize, sqlx::Type)]
pub enum CrawlStatus {
    Queued,
    Processing,
    Completed,
    Failed,
}

#[derive(Serialize)]
pub struct CrawlQueue {
    pub id: Option<i64>,
    /// URL to crawl
    pub url: String,
    /// When this was first added to the crawl queue
    pub created_at: DateTime<Utc>,
    pub status: CrawlStatus,
}

impl CrawlQueue {
    pub async fn init_table(db: &DbPool) -> anyhow::Result<(), sqlx::Error> {
        let mut conn = db.acquire().await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS crawl_queue (
                id INTEGER PRIMARY KEY,
                url TEXT UNIQUE,
                status TEXT,
                created_at DATETIME default CURRENT_TIMESTAMP
            )",
        )
        .execute(&mut conn)
        .await?;

        Ok(())
    }

    pub async fn insert(db: &DbPool, url: &str) -> anyhow::Result<(), sqlx::Error> {
        let mut conn = db.acquire().await?;

        sqlx::query("INSERT OR IGNORE INTO crawl_queue (url, status) VALUES (?, ?)")
            .bind(url)
            .bind(CrawlStatus::Queued)
            .execute(&mut conn)
            .await?;

        Ok(())
    }

    pub async fn list(db: &DbPool) -> anyhow::Result<Vec<CrawlQueue>, sqlx::Error> {
        let mut conn = db.acquire().await?;

        let results = sqlx::query(
            "SELECT id, url, status, created_at FROM crawl_queue LIMIT 100"
        ).fetch_all(&mut conn)
        .await?;

        let parsed = results
            .iter()
            .map(|row| CrawlQueue {
                id: row.get( 0),
                url: row.get::<String, _>(1),
                status: row.get(2),
                created_at: row.get(3),
            })
            .collect();

        Ok(parsed)
    }

    pub async fn next(db: &DbPool) -> anyhow::Result<Option<String>, sqlx::Error> {
        let mut conn = db.begin().await?;
        let row: Option<SqliteRow> = sqlx::query(
            "
                SELECT id, url
                FROM crawl_queue
                WHERE status = ?
                ORDER BY created_at ASC LIMIT 1",
        )
        .bind(CrawlStatus::Queued)
        .fetch_optional(&mut conn)
        .await?;

        if let Some(row) = row {
            let id: i64 = row.get(0);
            let url: String = row.get(1);

            sqlx::query("UPDATE crawl_queue SET status = ? WHERE id = ?")
                .bind(CrawlStatus::Processing)
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
