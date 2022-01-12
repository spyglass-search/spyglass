use chrono::prelude::*;
use rusqlite::{params, Connection};
pub struct CrawlQueue {
    pub id: Option<u64>,
    /// URL to crawl
    pub url: String,
    /// When this was first added to the crawl queue
    pub created_at: DateTime<Utc>,
}

impl CrawlQueue {
    pub fn init_table(db: &Connection) {
        db.execute(
            "CREATE TABLE IF NOT EXISTS crawl_queue (
                id INTEGER PRIMARY KEY,
                url TEXT UNIQUE,
                created_at DATETIME default CURRENT_TIMESTAMP
            )",
            [],
        )
        .expect("Unable to init `crawl_queue` table");
    }

    pub fn insert(db: &Connection, url: &str) -> Result<(), rusqlite::Error> {
        db.execute(
            "INSERT OR IGNORE INTO crawl_queue (url) VALUES (?1)",
            params![url],
        )?;

        Ok(())
    }
}