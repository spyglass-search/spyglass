use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use dashmap::DashMap;
use entities::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use notify::{event::ModifyKind, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;
use tokio::sync::{broadcast, mpsc};
use wasmer::{Instance, Module, Store, WasmerEnv};
use wasmer_wasi::{Pipe, WasiEnv, WasiState};

use entities::models::lens;
use shared::config::Config;
use shared::SettingOpts;
use spyglass_plugin::{consts::env, PluginEvent, PluginSubscription};

use crate::state::AppState;
use crate::task::AppShutdown;

mod exports;

#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub enum PluginType {
    /// A more complex lens than a simple list of URLs
    /// - Registers itself as a lens, under some "trigger" label.
    /// - Enqueues URLs to the crawl queue.
    /// - Can register to handle specific protocols if not HTTP
    Lens,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    pub name: String,
    pub author: String,
    pub description: String,
    pub version: String,
    #[serde(default)]
    pub path: Option<PathBuf>,
    pub plugin_type: PluginType,
    pub user_settings: HashMap<String, SettingOpts>,
    #[serde(default)]
    pub is_enabled: bool,
}

impl PluginConfig {
    pub fn data_folder(&self) -> PathBuf {
        self.path
            .as_ref()
            .expect("Unable to find plugin path")
            .parent()
            .expect("Unable to find parent plugin directory")
            .join("data")
    }
}

type PluginId = usize;
pub enum PluginCommand {
    DisablePlugin(String),
    EnablePlugin(String),
    Initialize(PluginConfig),
    // Request queued items from plugin
    HandleUpdate {
        plugin_id: PluginId,
        event: PluginEvent,
    },
    Subscribe(PluginId, PluginSubscription),
    // Queue up interval checks for subs
    QueueIntervalCheck,
    // Queue up file change notifications for subs
    QueueFileNotify(notify::Event),
}

/// Plugin context whenever we get a call from the one of the plugins
#[derive(WasmerEnv, Clone)]
pub(crate) struct PluginEnv {
    /// Id generated by the plugin manager
    id: PluginId,
    /// Name of the plugin
    name: String,
    /// Current application state
    app_state: AppState,
    /// Where the plugin stores data
    data_dir: PathBuf,
    /// wasi connection for communications
    wasi_env: WasiEnv,
    /// host specific requests
    cmd_writer: mpsc::Sender<PluginCommand>,
}

#[derive(Clone)]
struct PluginInstance {
    id: PluginId,
    config: PluginConfig,
    instance: Instance,
    env: WasiEnv,
}

impl PluginInstance {
    pub fn update(&mut self, event: PluginEvent) {
        if !self.config.is_enabled {
            return;
        }
        if let Ok(func) = self.instance.exports.get_function("update") {
            match wasi_write(&self.env, &event) {
                Err(e) => {
                    log::error!("unable to request update from plugin: {}", e)
                }
                Ok(_) => {
                    if let Err(e) = func.call(&[]) {
                        log::error!("update failed: {}", e);
                    }
                }
            }
        }
    }
}

struct PluginManager {
    check_update_subs: HashSet<PluginId>,
    file_watch_subs: DashMap<PluginId, String>,
    plugins: DashMap<PluginId, PluginInstance>,
    // For file watching subscribers
    file_events: Receiver<notify::Result<notify::Event>>,
    file_watcher: RecommendedWatcher,
}

impl PluginManager {
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(1);

        let watcher = RecommendedWatcher::new(move |res| {
            futures::executor::block_on(async {
                tx.send(res).await.expect("Unable to send FS event");
            })
        })
        .expect("Unable to watch lens directory");

        PluginManager {
            check_update_subs: Default::default(),
            file_watch_subs: Default::default(),
            plugins: Default::default(),
            file_events: rx,
            file_watcher: watcher,
        }
    }

    pub fn find_by_name(&self, name: String) -> Option<PluginInstance> {
        for entry in &self.plugins {
            if entry.config.name == name {
                return Some(entry.value().clone());
            }
        }

        None
    }
}

/// Manages plugin events
#[tracing::instrument(skip_all)]
pub async fn plugin_manager(
    state: AppState,
    config: Config,
    cmd_writer: mpsc::Sender<PluginCommand>,
    mut cmd_queue: mpsc::Receiver<PluginCommand>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    let mut config = config.clone();

    log::info!("plugin manager started");
    let mut manager = PluginManager::new();

    // Initial load, send some basic configuration to the plugins
    plugin_load(&state, &mut config, &cmd_writer).await;

    // Subscribe plugins check for updates every hour
    let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));

    loop {
        // Wait for next command / handle shutdown responses
        let next_cmd = tokio::select! {
            // Listen for plugin requests
            res = cmd_queue.recv() => res,
            // Listen for file change notifications
            file_event = manager.file_events.recv() => {
                if let Some(Ok(file_event)) = file_event {
                    Some(PluginCommand::QueueFileNotify(file_event))
                } else {
                    None
                }
            },
            // Handle interval checks
            _ = interval.tick() => Some(PluginCommand::QueueIntervalCheck),
            // SHUT IT DOWN
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down plugin manager");
                return;
            }
        };

        match next_cmd {
            Some(PluginCommand::DisablePlugin(plugin_name)) => {
                log::info!("disabling plugin <{}>", plugin_name);
                if let Some(plugin) = manager.find_by_name(plugin_name) {
                    if let Some(mut instance) = manager.plugins.get_mut(&plugin.id) {
                        instance.config.is_enabled = false;
                        manager.check_update_subs.remove(&plugin.id);
                    }
                }
            }
            Some(PluginCommand::EnablePlugin(plugin_name)) => {
                log::info!("enabling plugin <{}>", plugin_name);
                if let Some(plugin) = manager.find_by_name(plugin_name) {
                    if let Some(mut instance) = manager.plugins.get_mut(&plugin.id) {
                        instance.config.is_enabled = true;
                        // Re-initialize plugin
                        let _ = cmd_writer
                            .send(PluginCommand::Initialize(instance.config.clone()))
                            .await;
                    }
                }
            }
            Some(PluginCommand::HandleUpdate { plugin_id, event }) => {
                if let Some(mut plugin) = manager.plugins.get_mut(&plugin_id) {
                    plugin.update(event);
                } else {
                    log::error!("Unable to find plugin id: {}", plugin_id);
                }
            }
            Some(PluginCommand::Initialize(plugin)) => {
                let plugin_id = manager.plugins.len();
                match plugin_init(plugin_id, &state, &cmd_writer, &plugin).await {
                    Ok((instance, env)) => {
                        manager.plugins.insert(
                            plugin_id,
                            PluginInstance {
                                id: plugin_id,
                                config: plugin.clone(),
                                instance: instance.clone(),
                                env: env.clone(),
                            },
                        );
                    }
                    Err(e) => log::error!("Unable to init plugin <{}>: {}", plugin.name, e),
                }
            }
            Some(PluginCommand::Subscribe(plugin_id, event)) => match event {
                PluginSubscription::CheckUpdateInterval => {
                    manager.check_update_subs.insert(plugin_id);
                }
                PluginSubscription::WatchDirectory { path, recurse } => {
                    let dir_path = Path::new(&path);
                    // Ignore invalid directory paths
                    if !dir_path.exists() || !dir_path.is_dir() {
                        log::warn!("Ignoring invalid path: {}", path);
                        return;
                    }

                    let _ = manager.file_watcher.watch(
                        dir_path,
                        if recurse {
                            RecursiveMode::Recursive
                        } else {
                            RecursiveMode::NonRecursive
                        },
                    );

                    manager.file_watch_subs.insert(plugin_id, path);
                }
            },
            // Queue update checks for subscribed plugins
            Some(PluginCommand::QueueIntervalCheck) => {
                for plugin_id in &manager.check_update_subs {
                    let _ = cmd_writer
                        .send(PluginCommand::HandleUpdate {
                            plugin_id: *plugin_id,
                            event: PluginEvent::IntervalUpdate,
                        })
                        .await;
                }
            }
            // Notify subscribers of a new file event
            Some(PluginCommand::QueueFileNotify(file_event)) => {
                if !matches!(
                    file_event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                ) {
                    return;
                }

                for updated_path in file_event.paths {
                    let updated_path = updated_path.display().to_string();

                    let event = match &file_event.kind {
                        EventKind::Create(_) => {
                            Some(PluginEvent::FileCreated(updated_path.clone()))
                        }
                        EventKind::Modify(modify_kind) => match modify_kind {
                            ModifyKind::Any
                            | ModifyKind::Data(_)
                            | ModifyKind::Name(_)
                            | ModifyKind::Other => {
                                Some(PluginEvent::FileUpdated(updated_path.clone()))
                            }
                            ModifyKind::Metadata(_) => None,
                        },
                        EventKind::Remove(_) => {
                            Some(PluginEvent::FileDeleted(updated_path.clone()))
                        }
                        _ => None,
                    };

                    if let Some(event) = event {
                        for entry in &manager.file_watch_subs {
                            let watched_path = entry.value();
                            if updated_path.starts_with(watched_path) {
                                let _ = cmd_writer
                                    .send(PluginCommand::HandleUpdate {
                                        plugin_id: *entry.key(),
                                        event: event.clone(),
                                    })
                                    .await;
                            }
                        }
                    }
                }
            }
            None => {}
        }

        // Nothing to do
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await
    }
}

// Loop through plugins found in the plugins directory, enabling
pub async fn plugin_load(
    state: &AppState,
    config: &mut Config,
    cmds: &mpsc::Sender<PluginCommand>,
) {
    log::info!("🔌 loading plugins");

    let plugins_dir = config.plugins_dir();
    let plugin_files = fs::read_dir(plugins_dir).expect("Invalid plugin directory");

    for entry in plugin_files.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Load plugin settings
            let plugin_config = path.join("manifest.ron");
            if !plugin_config.exists() || !plugin_config.is_file() {
                log::warn!("Invalid plugin manifest: {}", path.as_path().display());
                continue;
            }

            match fs::read_to_string(plugin_config) {
                Ok(file_contents) => match ron::from_str::<PluginConfig>(&file_contents) {
                    // Successfully loaded plugin manifest
                    Ok(plug) => {
                        let mut plug = plug.clone();
                        plug.path = Some(path.join("main.wasm"));
                        // If any user settings are found, override default ones
                        // from plugin config file.
                        let user_settings = config
                            .plugin_settings
                            .entry(plug.name.clone())
                            .or_insert_with(HashMap::new);

                        // Loop through plugin settings and use any user overrides found.
                        for (key, value) in plug.user_settings.iter_mut() {
                            let user_override = user_settings
                                .entry(key.to_string())
                                .or_insert_with(|| value.value.to_string());
                            (*value).value = user_override.to_string();
                        }
                        // Update the user settings file in case any new setting entries
                        // were added.
                        let _ = config.save_plugin_settings(&config.plugin_settings);

                        // Enable plugins that are lenses, this is the only type right so technically they
                        // all will be enabled as a lens.
                        if plug.plugin_type == PluginType::Lens {
                            match lens::add_or_enable(
                                &state.db,
                                &plug.name,
                                &plug.author,
                                Some(&plug.description),
                                &plug.version,
                                lens::LensType::Plugin,
                            )
                            .await
                            {
                                Ok(is_new) => {
                                    log::info!("loaded lens {}, new? {}", plug.name, is_new)
                                }
                                Err(e) => log::error!("Unable to add lens: {}", e),
                            }
                        }

                        // Is this plugin enabled?
                        let lens_config = lens::Entity::find()
                            .filter(lens::Column::Name.eq(plug.name.clone()))
                            .one(&state.db)
                            .await;

                        if let Ok(Some(lens_config)) = lens_config {
                            plug.is_enabled = lens_config.is_enabled;
                        }

                        if cmds
                            .send(PluginCommand::Initialize(plug.clone()))
                            .await
                            .is_ok()
                        {
                            log::info!("<{}> plugin found", &plug.name);
                        }
                    }
                    Err(e) => log::error!("Couldn't parse plugin config: {}", e),
                },
                Err(e) => log::error!("Couldn't read plugin config: {}", e),
            }
        }
    }
}

pub async fn plugin_init(
    plugin_id: PluginId,
    state: &AppState,
    cmd_writer: &mpsc::Sender<PluginCommand>,
    plugin: &PluginConfig,
) -> anyhow::Result<(Instance, WasiEnv)> {
    if plugin.path.is_none() {
        // Nothing to do if theres no WASM file to load.
        return Err(anyhow::Error::msg(format!(
            "Unable to find plugin path: {:?}",
            plugin.path
        )));
    }

    // Make sure data folder exists
    std::fs::create_dir_all(plugin.data_folder()).expect("Unable to create plugin data folder");

    let path = plugin.path.as_ref().expect("Unable to extract plugin path");
    let output = Pipe::new();
    let input = Pipe::new();

    let store = Store::default();
    let module = Module::from_file(&store, &path)?;
    let user_settings = &plugin.user_settings;

    // Detect base data dir and send that to the plugin
    let base_config_dir = directories::BaseDirs::new()
        .map(|base| base.config_dir().display().to_string())
        .map_or_else(|| "".to_string(), |dir| dir);

    let base_data_dir: String = directories::BaseDirs::new()
        .map(|base| base.data_local_dir().display().to_string())
        .map_or_else(|| "".to_string(), |dir| dir);

    let home_dir: String = directories::BaseDirs::new()
        .map(|base| base.home_dir().display().to_string())
        .map_or_else(|| "".to_string(), |dir| dir);

    let mut wasi_env = WasiState::new(&plugin.name)
        // Attach the plugin data directory. Anything created by the plugin will live
        // there.
        .map_dir("/", plugin.data_folder())
        .expect("Unable to mount plugin data folder")
        .env(env::BASE_CONFIG_DIR, base_config_dir)
        .env(env::BASE_DATA_DIR, base_data_dir)
        .env(env::HOST_HOME_DIR, home_dir)
        .env(env::HOST_OS, std::env::consts::OS)
        // Load user settings as environment variables
        .envs(
            user_settings
                .iter()
                .map(|(name, opts)| (name, opts.value.clone())),
        )
        // Override stdin/out with pipes for comms
        .stdin(Box::new(input))
        .stdout(Box::new(output))
        .finalize()?;

    let mut import_object = wasi_env.import_object(&module)?;
    // Register exported functions
    import_object.register(
        "spyglass",
        exports::register_exports(plugin_id, state, cmd_writer, plugin, &store, &wasi_env),
    );

    // Instantiate the module wn the imports
    let instance = Instance::new(&module, &import_object)?;

    // Lets call the `_start` function, which is our `main` function in Rust
    if plugin.is_enabled {
        log::info!("STARTING <{}>", plugin.name);
        let start = instance.exports.get_function("_start")?;
        start.call(&[])?;
    }

    Ok((instance, wasi_env))
}

// --------------------------------------------------------------------------------
// Utility functions for wasi <> spyglass comms
// --------------------------------------------------------------------------------

fn wasi_read_string(wasi_env: &WasiEnv) -> anyhow::Result<String> {
    let mut state = wasi_env.state();
    let stdout = state
        .fs
        .stdout_mut()?
        .as_mut()
        .ok_or_else(|| anyhow::Error::msg("Unable to unwrap stdout"))?;

    let mut buf = String::new();
    stdout.read_to_string(&mut buf)?;
    let buf = buf.trim().to_string();
    Ok(buf)
}

fn wasi_write_string(env: &WasiEnv, buf: &str) -> anyhow::Result<()> {
    let mut state = env.state();
    let stdin = state
        .fs
        .stdin_mut()?
        .as_mut()
        .ok_or_else(|| anyhow::Error::msg("Unable to get stdin pipe"))?;
    writeln!(stdin, "{}\r", buf)?;
    Ok(())
}

fn wasi_read<T: DeserializeOwned>(env: &WasiEnv) -> anyhow::Result<T> {
    let buf = wasi_read_string(env)?;
    Ok(ron::from_str(&buf)?)
}

fn wasi_write(env: &WasiEnv, obj: &(impl Serialize + ?Sized)) -> anyhow::Result<()> {
    wasi_write_string(env, &ron::to_string(&obj)?)
}
