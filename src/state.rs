use std::fs;
use std::path::PathBuf;

use sea_orm::DatabaseConnection;

use crate::config::Config;
use crate::models::{create_connection, setup_schema};
use crate::search::{IndexPath, Searcher};

pub struct AppState {
    pub db: DatabaseConnection,
    pub config: Config,
    pub index: Searcher,
}

impl AppState {
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

        let db = create_connection(&config, false)
            .await
            .expect("Unable to connect to database");

        let index = Searcher::with_index(&IndexPath::LocalPath(Self::index_dir()));

        let app = AppState {
            db: db.clone(),
            config,
            index,
        };
        let _ = setup_schema(&db.clone()).await;
        app.init_data_folders().await;

        app
    }
}
