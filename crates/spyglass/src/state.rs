use std::sync::{Arc, Mutex};

use dashmap::DashMap;
use shared::config::Config;
use shared::sea_orm::DatabaseConnection;

use crate::search::{IndexPath, Searcher};
use shared::models::create_connection;

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
        app_state.insert("paused".to_string(), "false".to_string());

        AppState {
            db,
            app_state: Arc::new(app_state),
            config,
            index: Arc::new(Mutex::new(index)),
        }
    }
}
