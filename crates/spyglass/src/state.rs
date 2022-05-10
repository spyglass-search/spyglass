use std::sync::{Arc, Mutex};

use dashmap::DashMap;
use entities::models::create_connection;
use entities::sea_orm::DatabaseConnection;
use shared::config::{Config, UserSettings, Lens};
use crate::search::{IndexPath, Searcher};

#[derive(Debug, Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub app_state: Arc<DashMap<String, String>>,
    pub lenses: Arc<DashMap<String, Lens>>,
    pub user_settings: UserSettings,
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

        // Convert into dashmap
        let lenses = DashMap::new();
        for (key, value) in config.lenses.into_iter() {
            lenses.insert(key, value);
        }

        AppState {
            db,
            app_state: Arc::new(app_state),
            user_settings: config.user_settings,
            lenses: Arc::new(lenses),
            index: Arc::new(Mutex::new(index)),
        }
    }
}
