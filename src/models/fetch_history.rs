#![allow(dead_code)]

use chrono::prelude::*;
use rusqlite::{params, Connection};

/// When a URL was last fetched. Also used as a queue for the indexer to determine
/// what paths to index next.
#[derive(Debug)]
pub struct FetchHistory {
    /// Arbitrary id for this.
    pub id: Option<u64>,
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
    pub fn init_table(db: &Connection) {
        db.execute(
            "CREATE TABLE IF NOT EXISTS fetch_history (
                id INTEGER PRIMARY KEY,
                url TEXT UNIQUE,
                hash TEXT,
                status INTEGER,
                no_index BOOLEAN,
                created_at DATETIME default CURRENT_TIMESTAMP,
                updated_at DATETIME default CURRENT_TIMESTAMP
            )",
            [],
        )
        .expect("Unable to init `fetch_history` table");
    }

    pub fn find(db: &Connection, url: &str) -> Result<Option<FetchHistory>, rusqlite::Error> {
        let mut stmt = db.prepare(
            "SELECT
                id,
                url,
                hash,
                status,
                no_index,
                created_at,
                updated_at
                FROM fetch_history WHERE url = ?",
        )?;

        if !stmt.exists(params![url])? {
            return Ok(None);
        }

        let row = stmt.query_row(params![url], |row| {
            Ok(FetchHistory {
                id: Some(row.get(0)?),
                url: row.get(1)?,
                hash: row.get(2)?,
                status: row.get(3)?,
                no_index: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;

        Ok(Some(row))
    }

    pub fn insert(
        db: &Connection,
        url: &str,
        hash: Option<String>,
        status: u16,
    ) -> Result<(), rusqlite::Error> {
        db.execute(
            "INSERT INTO fetch_history (url, hash, status, no_index)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(url) DO UPDATE SET
                    updated_at = CURRENT_TIMESTAMP,
                    hash = ?2
                ",
            params![url, hash, status, false],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::models::FetchHistory;
    use rusqlite::Connection;

    #[test]
    fn test_init() {
        let db = Connection::open_in_memory().unwrap();
        FetchHistory::init_table(&db);
    }

    #[test]
    fn test_insert() {
        let db = Connection::open_in_memory().unwrap();
        FetchHistory::init_table(&db);

        let hash = "this is a hash".to_string();
        FetchHistory::insert(&db, "oldschool.runescape.wiki/", Some(hash.clone()), 200).unwrap();

        let url = "oldschool.runescape.wiki/";
        let history = FetchHistory::find(&db, url).unwrap().unwrap();
        assert_eq!(history.url, url);
        assert_eq!(history.hash.unwrap(), hash);
    }
}
