use std::sync::Arc;

use dashmap::DashMap;
use entities::models::create_connection;
use entities::sea_orm::DatabaseConnection;
use tokio::sync::Mutex;
use tokio::sync::{broadcast, mpsc};

use crate::{
    plugin::{PluginCommand, PluginManager},
    search::{IndexPath, Searcher},
    task::Command,
};
use shared::config::{Config, LensConfig, UserSettings};

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub app_state: Arc<DashMap<String, String>>,
    pub lenses: Arc<DashMap<String, LensConfig>>,
    pub user_settings: UserSettings,
    pub index: Searcher,
    // Crawler pause control
    pub crawler_cmd_tx: Arc<Mutex<Option<broadcast::Sender<Command>>>>,
    // Plugin command/control
    pub plugin_cmd_tx: Arc<Mutex<Option<mpsc::Sender<PluginCommand>>>>,
    pub plugin_manager: Arc<Mutex<PluginManager>>,
}

impl AppState {
    pub async fn new(config: &Config) -> Self {
        let db = create_connection(config, false)
            .await
            .expect("Unable to connect to database");

        let index = Searcher::with_index(&IndexPath::LocalPath(config.index_dir()));

        // TODO: Load from saved preferences
        let app_state = DashMap::new();
        app_state.insert("paused".to_string(), "false".to_string());

        // Convert into dashmap
        let lenses = DashMap::new();
        for (key, value) in config.lenses.iter() {
            lenses.insert(key.clone(), value.clone());
        }

        AppState {
            db,
            app_state: Arc::new(app_state),
            user_settings: config.user_settings.clone(),
            lenses: Arc::new(lenses),
            index,
            crawler_cmd_tx: Arc::new(Mutex::new(None)),
            plugin_cmd_tx: Arc::new(Mutex::new(None)),
            plugin_manager: Arc::new(Mutex::new(PluginManager::new())),
        }
    }

    pub fn builder() -> AppStateBuilder {
        AppStateBuilder::new()
    }
}

#[derive(Default)]
pub struct AppStateBuilder {
    db: Option<DatabaseConnection>,
    index: Option<Searcher>,
    lenses: Option<Vec<LensConfig>>,
    user_settings: Option<UserSettings>,
}

impl AppStateBuilder {
    pub fn build(&self) -> AppState {
        let lenses = DashMap::new();
        if let Some(res) = &self.lenses {
            for lens in res {
                lenses.insert(lens.name.clone(), lens.to_owned());
            }
        }

        AppState {
            app_state: Arc::new(DashMap::new()),
            db: self.db.as_ref().expect("Must set db").to_owned(),
            user_settings: self
                .user_settings
                .as_ref()
                .expect("Must set user settings")
                .to_owned(),
            index: self.index.as_ref().expect("Must set index").to_owned(),
            lenses: Arc::new(lenses),
            crawler_cmd_tx: Arc::new(Mutex::new(None)),
            plugin_cmd_tx: Arc::new(Mutex::new(None)),
            plugin_manager: Arc::new(Mutex::new(PluginManager::new())),
        }
    }

    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_db(&mut self, db: DatabaseConnection) -> &mut Self {
        self.db = Some(db);
        self
    }

    pub fn with_lenses(&mut self, lenses: &Vec<LensConfig>) -> &mut Self {
        self.lenses = Some(lenses.to_owned());
        self
    }

    pub fn with_user_settings(&mut self, user_settings: &UserSettings) -> &mut Self {
        self.user_settings = Some(user_settings.to_owned());
        self
    }

    pub fn with_index(&mut self, index: &IndexPath) -> &mut Self {
        self.index = Some(Searcher::with_index(index));
        self
    }
}
