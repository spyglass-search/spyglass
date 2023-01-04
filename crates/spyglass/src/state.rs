use std::sync::Arc;

use dashmap::DashMap;
use entities::models::create_connection;
use entities::sea_orm::DatabaseConnection;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::Mutex;
use tokio::sync::{broadcast, mpsc};

use crate::task::AppShutdown;
use crate::{
    pipeline::PipelineCommand,
    plugin::{PluginCommand, PluginManager},
    search::{IndexPath, Searcher},
    task::{AppPause, ManagerCommand},
};
use shared::config::{Config, LensConfig, PipelineConfiguration, UserSettings};
use shared::metrics::Metrics;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub app_state: Arc<DashMap<String, String>>,
    pub lenses: Arc<DashMap<String, LensConfig>>,
    pub pipelines: Arc<DashMap<String, PipelineConfiguration>>,
    pub user_settings: UserSettings,
    pub index: Searcher,
    pub metrics: Metrics,
    // Task scheduler command/control
    pub manager_cmd_tx: Arc<Mutex<Option<mpsc::UnboundedSender<ManagerCommand>>>>,
    pub shutdown_cmd_tx: Arc<Mutex<broadcast::Sender<AppShutdown>>>,
    // Pause/unpause worker pool.
    pub pause_cmd_tx: Arc<Mutex<Option<broadcast::Sender<AppPause>>>>,
    // Plugin command/control
    pub plugin_cmd_tx: Arc<Mutex<Option<mpsc::Sender<PluginCommand>>>>,
    pub plugin_manager: Arc<Mutex<PluginManager>>,
    // Pipeline command/control
    pub pipeline_cmd_tx: Arc<Mutex<Option<mpsc::Sender<PipelineCommand>>>>,
}

impl AppState {
    pub async fn new(config: &Config) -> Self {
        let db = create_connection(config, false)
            .await
            .expect("Unable to connect to database");

        log::debug!("Loading index from: {:?}", config.index_dir());
        let index = Searcher::with_index(&IndexPath::LocalPath(config.index_dir()))
            .expect("Unable to open index.");

        // TODO: Load from saved preferences
        let app_state = DashMap::new();
        app_state.insert("paused".to_string(), "false".to_string());

        // Convert into dashmap
        let lenses = DashMap::new();
        for (key, value) in config.lenses.iter() {
            lenses.insert(key.clone(), value.clone());
        }

        let pipelines = DashMap::new();
        for (key, value) in config.pipelines.iter() {
            pipelines.insert(key.clone(), value.clone());
        }

        let (shutdown_tx, _) = broadcast::channel::<AppShutdown>(16);

        AppState {
            db,
            app_state: Arc::new(app_state),
            user_settings: config.user_settings.clone(),
            metrics: Metrics::new(
                &Config::machine_identifier(),
                config.user_settings.disable_telemetry,
            ),
            lenses: Arc::new(lenses),
            pipelines: Arc::new(pipelines),
            index,
            shutdown_cmd_tx: Arc::new(Mutex::new(shutdown_tx)),
            pause_cmd_tx: Arc::new(Mutex::new(None)),
            plugin_cmd_tx: Arc::new(Mutex::new(None)),
            pipeline_cmd_tx: Arc::new(Mutex::new(None)),
            plugin_manager: Arc::new(Mutex::new(PluginManager::new())),
            manager_cmd_tx: Arc::new(Mutex::new(None)),
        }
    }

    pub fn builder() -> AppStateBuilder {
        AppStateBuilder::new()
    }

    pub async fn schedule_work(
        &self,
        task: ManagerCommand,
    ) -> Result<(), SendError<ManagerCommand>> {
        let cmd_tx = self.manager_cmd_tx.lock().await;
        let cmd_tx = cmd_tx.as_ref().expect("Manager channel not open");
        cmd_tx.send(task)
    }
}

#[derive(Default)]
pub struct AppStateBuilder {
    db: Option<DatabaseConnection>,
    index: Option<Searcher>,
    lenses: Option<Vec<LensConfig>>,
    pipelines: Option<Vec<PipelineConfiguration>>,
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

        let pipelines = DashMap::new();
        if let Some(res) = &self.pipelines {
            for pipeline in res {
                pipelines.insert(pipeline.kind.clone(), pipeline.to_owned());
            }
        }

        let index = if let Some(index) = &self.index {
            index.to_owned()
        } else {
            Searcher::with_index(&IndexPath::Memory).expect("Unable to open search index")
        };

        let user_settings = if let Some(settings) = &self.user_settings {
            settings.to_owned()
        } else {
            UserSettings::default()
        };

        let (shutdown_tx, _) = broadcast::channel::<AppShutdown>(16);

        AppState {
            app_state: Arc::new(DashMap::new()),
            db: self.db.as_ref().expect("Must set db").to_owned(),
            index,
            lenses: Arc::new(lenses),
            manager_cmd_tx: Arc::new(Mutex::new(None)),
            metrics: Metrics::new(
                &Config::machine_identifier(),
                user_settings.disable_telemetry,
            ),
            pause_cmd_tx: Arc::new(Mutex::new(None)),
            pipeline_cmd_tx: Arc::new(Mutex::new(None)),
            pipelines: Arc::new(pipelines),
            plugin_cmd_tx: Arc::new(Mutex::new(None)),
            plugin_manager: Arc::new(Mutex::new(PluginManager::new())),
            shutdown_cmd_tx: Arc::new(Mutex::new(shutdown_tx)),
            user_settings,
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
        self.index = Some(Searcher::with_index(index).expect("Unable to open index"));
        self
    }
}
