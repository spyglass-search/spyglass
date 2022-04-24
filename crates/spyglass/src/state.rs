use std::sync::{Arc, Mutex};

use dashmap::DashMap;
use sea_orm::DatabaseConnection;
use shared::config::Config;

use crate::models::{create_connection, setup_schema};
use crate::search::{IndexPath, Searcher};

#[derive(Debug, Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub app_state: Arc<DashMap<String, String>>,
    pub config: Config,
    pub index: Arc<Mutex<Searcher>>,
}

impl AppState {
    pub async fn new() -> Self {
        let config = Config::new();

        let db = create_connection(false)
            .await
            .expect("Unable to connect to database");

        let index = Searcher::with_index(&IndexPath::LocalPath(Config::index_dir()));

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

        app
    }
}
