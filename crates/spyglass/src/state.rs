use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use dashmap::DashMap;
use sea_orm::DatabaseConnection;
use shared::config::Config;

use crate::models::{create_connection, setup_schema};
use crate::search::{IndexPath, Searcher};

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub app_state: Arc<DashMap<String, String>>,
    pub config: Config,
    pub index: Arc<Mutex<Searcher>>,
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
        let db = create_connection(false)
            .await
            .expect("Unable to connect to database");

        let index = Searcher::with_index(&IndexPath::LocalPath(Self::index_dir()));

        // TODO: Load from saved preferences
        let app_state = DashMap::new();
        app_state.insert("paused".to_string(), "true".to_string());

        let app = AppState {
            db: db.clone(),
            app_state: Arc::new(app_state),
            config,
            index: Arc::new(Mutex::new(index)),
        };
        let _ = setup_schema(&db.clone())
            .await
            .expect("Unable to setup schema");
        app.init_data_folders().await;

        app
    }
}
