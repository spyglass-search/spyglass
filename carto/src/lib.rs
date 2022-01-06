use directories::ProjectDirs;
use reqwest::StatusCode;
use rusqlite::{Connection, OpenFlags, Result};
use std::{fs, path::PathBuf};

pub mod models;
use models::Place;

pub struct Carto {
    db: Connection,
    data_dir: PathBuf,
}

impl Carto {
    pub fn init_db(&self, ) -> Result<()> {
        // Initialize robots table
        self.db.execute(
            "CREATE TABLE IF NOT EXISTS robots_txt (
                id INTEGER PRIMARY KEY,
                domain TEXT UNIQUE,
                no_index BOOLEAN,
                disallow TEXT,
                allow TEXT,
                created_at DATETIME,
                updated_at DATETIME
            )", []
        )?;

        // Initialize fetch history table
        self.db.execute(
            "CREATE TABLE IF NOT EXISTS fetch_history(
                id INTEGER PRIMARY KEY,
                url TEXT UNIQUE,
                hash TEXT,
                status INTEGER,
                no_index BOOLEAN,
                created_at DATETIME,
                updated_at DATETIME
            )", []
        )?;

        Ok(())
    }

    pub fn init() -> Self {
        let proj_dirs = ProjectDirs::from("com", "athlabs", "carto").unwrap();
        let data_dir = proj_dirs.data_dir().join("crawls");

        fs::create_dir_all(&data_dir).expect("Unable to create crawl folder");

        let db_path = proj_dirs.data_dir().join("db.sqlite");
        dbg!(&db_path);
        let db = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE |
            OpenFlags::SQLITE_OPEN_CREATE
        ).unwrap();

        let carto = Carto { db, data_dir };
        carto.init_db().expect("Unable to initialize db");

        carto
    }

    // TODO: Load web indexing as a plugin?
    pub async fn fetch(&self, place: &Place) {
        // Make sure cache directory exists for this domain
        let url = &place.url;
        let domain_dir = self.data_dir.join(url.host_str().unwrap());
        if !domain_dir.exists() {
            fs::create_dir(&domain_dir).expect("Unable to create dir");
        }

        // Check for robots.txt of this domain

        log::info!("Fetching page: {}", place.url.as_str());
        let res = reqwest::get(place.url.as_str()).await.unwrap();
        log::info!("Status: {}", res.status());
        if res.status() == StatusCode::OK {
            // TODO: Save headers
            log::info!("Headers:\n{:?}", res.headers());
            let body = res.text().await.unwrap();
            let file_path = domain_dir.join("raw.html");
            fs::write(file_path, body).expect("Unable to save html");
        }

        // Update fetch history

    }
}
