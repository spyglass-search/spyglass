use arc_swap::ArcSwap;
use dashmap::DashMap;
use entities::models::create_connection;
use entities::sea_orm::DatabaseConnection;
use spyglass_model_interface::embedding_api::EmbeddingApi;
use spyglass_rpc::RpcEvent;
use spyglass_searcher::schema::DocFields;
use spyglass_searcher::schema::SearchDocument;
use std::sync::Arc;
use tantivy::schema::Schema;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::Mutex;
use tokio::sync::{broadcast, mpsc};

use crate::filesystem::SpyglassFileWatcher;
use crate::task::{AppShutdown, UserSettingsChange};
use crate::{
    pipeline::PipelineCommand,
    task::{AppPause, ManagerCommand},
};
use shared::config::{Config, LensConfig, PipelineConfiguration, UserSettings};
use shared::metrics::Metrics;
use spyglass_searcher::{client::Searcher, IndexBackend};

/// Used to track inflight requests and limit things
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum FetchLimitType {
    Audio,
    File,
}

impl FetchLimitType {
    pub async fn check_and_wait(
        fetch_limits: &DashMap<FetchLimitType, usize>,
        limit_type: Self,
        limit: usize,
        wait_log: &str,
    ) {
        {
            if !fetch_limits.contains_key(&limit_type) {
                fetch_limits.insert(limit_type.clone(), 0);
            }
        }

        while let Some(inflight) = fetch_limits.view(&limit_type, |_, v| *v) {
            if inflight >= limit {
                log::debug!("{wait_log}");
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            } else {
                fetch_limits.alter(&limit_type, |_, v| v + 1);
                break;
            }
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub embedding_api: Arc<ArcSwap<Option<EmbeddingApi>>>,
    pub app_state: Arc<DashMap<String, String>>,
    pub lenses: Arc<DashMap<String, LensConfig>>,
    pub pipelines: Arc<DashMap<String, PipelineConfiguration>>,
    pub user_settings: Arc<ArcSwap<UserSettings>>,
    pub index: Searcher,
    pub metrics: Metrics,
    pub config: Config,
    // Task scheduler command/control
    pub manager_cmd_tx: Arc<Mutex<Option<mpsc::UnboundedSender<ManagerCommand>>>>,
    pub shutdown_cmd_tx: Arc<Mutex<broadcast::Sender<AppShutdown>>>,
    pub config_cmd_tx: Arc<Mutex<broadcast::Sender<UserSettingsChange>>>,
    // Client events
    pub rpc_events: Arc<std::sync::Mutex<broadcast::Sender<RpcEvent>>>,
    // Pause/unpause worker pool.
    pub pause_cmd_tx: Arc<Mutex<Option<broadcast::Sender<AppPause>>>>,
    // Pipeline command/control
    pub pipeline_cmd_tx: Arc<Mutex<Option<mpsc::Sender<PipelineCommand>>>>,
    pub file_watcher: Arc<Mutex<Option<SpyglassFileWatcher>>>,
    // Keep track of in-flight tasks
    pub fetch_limits: Arc<DashMap<FetchLimitType, usize>>,
    pub readonly_mode: bool,
}

impl AppState {
    pub async fn new(config: &Config, readonly_mode: bool) -> Self {
        let db_connection_result = create_connection(config, false).await;
        if let Err(error) = &db_connection_result {
            log::error!("Error connecting to database {:?}", error);
        }

        let db = db_connection_result.expect("Unable to connect to database");

        AppStateBuilder::new()
            .with_db(db)
            .with_index(
                &IndexBackend::LocalPath(config.index_dir()),
                DocFields::as_schema(),
                readonly_mode,
            )
            .with_lenses(&config.lenses.values().cloned().collect())
            .with_pipelines(
                &config
                    .pipelines
                    .values()
                    .cloned()
                    .collect::<Vec<PipelineConfiguration>>(),
            )
            .with_user_settings(&config.user_settings)
            .build()
    }

    pub fn reload_config(&mut self) {
        log::debug!("reloading config...");
        let config = Config::new();

        self.user_settings
            .store(Arc::new(config.user_settings.clone()));

        // self.user_settings = config.user_settings.clone();
        self.config = config;
    }

    pub fn reload_model(&mut self) {
        let embedding_api = load_model(self.user_settings.load_full().as_ref());
        self.embedding_api.store(Arc::new(embedding_api));
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

    pub async fn publish_event(&self, event: &RpcEvent) {
        log::debug!("publishing event: {:?}", event);
        let rpc_sub = self.rpc_events.lock().unwrap();
        // no use sending if no one is listening.
        if rpc_sub.receiver_count() > 0 {
            if let Err(err) = rpc_sub.send(event.clone()) {
                log::warn!("error sending event: {:?}", err);
            }
        }
    }
}

#[derive(Default)]
pub struct AppStateBuilder {
    db: Option<DatabaseConnection>,
    index: Option<Searcher>,
    lenses: Option<Vec<LensConfig>>,
    pipelines: Option<Vec<PipelineConfiguration>>,
    user_settings: Option<UserSettings>,
    readonly_mode: Option<bool>,
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
            Searcher::with_index(&IndexBackend::Memory, DocFields::as_schema(), false)
                .expect("Unable to open search index")
        };

        let user_settings = if let Some(settings) = &self.user_settings {
            settings.to_owned()
        } else {
            UserSettings::default()
        };

        let embedding_api = load_model(&user_settings);

        let (shutdown_tx, _) = broadcast::channel::<AppShutdown>(16);
        let (config_tx, _) = broadcast::channel::<UserSettingsChange>(16);
        let (rpc_events, _) = broadcast::channel::<RpcEvent>(10);

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
            config: Config::new(),
            pause_cmd_tx: Arc::new(Mutex::new(None)),
            pipeline_cmd_tx: Arc::new(Mutex::new(None)),
            pipelines: Arc::new(pipelines),
            rpc_events: Arc::new(std::sync::Mutex::new(rpc_events)),
            shutdown_cmd_tx: Arc::new(Mutex::new(shutdown_tx)),
            config_cmd_tx: Arc::new(Mutex::new(config_tx)),
            file_watcher: Arc::new(Mutex::new(None)),
            user_settings: Arc::new(ArcSwap::from_pointee(user_settings)),
            fetch_limits: Arc::new(DashMap::new()),
            readonly_mode: self.readonly_mode.unwrap_or_default(),
            embedding_api: Arc::new(ArcSwap::from_pointee(embedding_api)),
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

    pub fn with_pipelines(&mut self, pipelines: &[PipelineConfiguration]) -> &mut Self {
        self.pipelines = Some(pipelines.to_owned());
        self
    }

    pub fn with_user_settings(&mut self, user_settings: &UserSettings) -> &mut Self {
        self.user_settings = Some(user_settings.to_owned());
        self
    }

    pub fn with_index(
        &mut self,
        index: &IndexBackend,
        schema: Schema,
        readonly: bool,
    ) -> &mut Self {
        if let IndexBackend::LocalPath(path) = &index {
            if !path.exists() {
                let _ = std::fs::create_dir_all(path);
            }
        }

        let searcher = Searcher::with_index(index, schema, readonly);
        if let Err(error) = &searcher {
            log::error!("Error connecting to index {index:?}. Error: {error:?}");
        }

        self.index = Some(searcher.expect("Unable to open index"));
        self
    }
}

fn load_model(user_settings: &UserSettings) -> Option<EmbeddingApi> {
    if user_settings.embedding_settings.enable_embeddings {
        let mut model_root = user_settings.data_directory.clone();
        model_root.push("models");
        model_root.push("embeddings");

        let mut tokenizer_file = model_root.clone();
        tokenizer_file.push("tokenizer.json");
        let mut model = model_root.clone();
        model.push("model.safetensors");

        if tokenizer_file.exists() && model.exists() {
            match EmbeddingApi::new(model_root.clone()) {
                Ok(embedding_api) => {
                    log::info!("Embedding Model Loaded");
                    Some(embedding_api)
                }
                Err(error) => {
                    log::error!("Error Loading Embedding Model {:?}", error);
                    None
                }
            }
        } else {
            log::warn!("Model does not exist");
            None
        }
    } else {
        None
    }
}
