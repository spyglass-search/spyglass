use std::fs;
use std::path::PathBuf;

use crate::config::Config;
use crate::models::{create_connection, CrawlQueue, DbPool, FetchHistory, ResourceRule};

pub struct AppState {
    pub conn: DbPool,
    pub config: Config,
}

impl AppState {
    /// Initialize db tables
    pub async fn init_db(&self) {
        CrawlQueue::init_table(&self.conn).await.unwrap();
        ResourceRule::init_table(&self.conn).await.unwrap();
        FetchHistory::init_table(&self.conn).await.unwrap();
    }

    pub fn crawl_dir() -> PathBuf {
        Config::data_dir().join("crawls")
    }

    pub fn index_dir() -> PathBuf {
        Config::data_dir().join("index")
    }

    pub async fn init_data_folders(&self) {
        fs::create_dir_all(AppState::crawl_dir()).expect("Unable to create crawl folder");
        fs::create_dir_all(AppState::index_dir()).expect("Unable to create index folder");
    }

    pub async fn new() -> Self {
        let config = Config::new();
        log::info!("config: {:?}", config);

        let conn = create_connection(&config)
            .await
            .expect("Unable to connect to database");

        let app = AppState { conn, config };
        app.init_db().await;
        app.init_data_folders().await;

        app
    }
}
