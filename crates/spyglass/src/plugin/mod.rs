use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

use dashmap::DashMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use shared::config::Config;
use tokio::sync::{broadcast, mpsc};
use wasmer::{Instance, Module, Store, WasmerEnv};
use wasmer_wasi::{Pipe, WasiEnv, WasiState};

use crate::state::AppState;
use crate::task::AppShutdown;

mod exports;

#[derive(Clone, Deserialize, Serialize)]
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
    #[serde(default)]
    pub path: Option<PathBuf>,
    pub plugin_type: PluginType,
    pub user_settings: HashMap<String, String>,
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
    Initialize(PluginConfig),
    // Request queued items from plugin
    RequestQueue(PluginId),
}

// Basic environment information for the plugin
#[derive(WasmerEnv, Clone)]
pub(crate) struct PluginEnv {
    name: String,
    app_state: AppState,
    data_dir: PathBuf,
    wasi_env: WasiEnv,
}

#[derive(Clone)]
struct PluginInstance {
    #[allow(dead_code)]
    id: PluginId,
    #[allow(dead_code)]
    config: PluginConfig,
    instance: Instance,
}

#[derive(Default)]
struct PluginManager {
    plugins: DashMap<PluginId, PluginInstance>,
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
    log::info!("plugin manager started");
    let manager = PluginManager::default();

    // Initial load, send some basic configuration to the plugins
    plugin_load(config, &cmd_writer).await;

    loop {
        // Wait for next command / handle shutdown responses
        let next_cmd = tokio::select! {
            res = cmd_queue.recv() => res,
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down worker");
                return;
            }
        };

        match next_cmd {
            Some(PluginCommand::Initialize(plugin)) => match plugin_init(&state, &plugin) {
                Ok(instance) => {
                    let plugin_id = manager.plugins.len();
                    manager.plugins.insert(
                        plugin_id,
                        PluginInstance {
                            id: plugin_id,
                            config: plugin.clone(),
                            instance: instance.clone(),
                        },
                    );
                    let _ = cmd_writer
                        .send(PluginCommand::RequestQueue(plugin_id))
                        .await;
                }
                Err(e) => log::error!("Unable to init plugin <{}>: {}", plugin.name, e),
            },
            Some(PluginCommand::RequestQueue(plugin_id)) => {
                if let Some(plugin) = manager.plugins.get(&plugin_id) {
                    if let Ok(func) = plugin.instance.exports.get_function("request_queue") {
                        if let Err(e) = func.call(&[]) {
                            log::error!("request_queue failed: {}", e);
                        }
                    }
                } else {
                    log::error!("Unable to find plugin id: {}", plugin_id);
                }
            }
            // Nothing to do
            _ => tokio::time::sleep(tokio::time::Duration::from_secs(1)).await,
        }
    }
}

pub async fn plugin_load(config: Config, cmds: &mpsc::Sender<PluginCommand>) {
    log::info!("🔌 loading plugins");

    let plugins_dir = config.plugins_dir();
    let plugin_files = fs::read_dir(plugins_dir).expect("Invalid plugin directory");

    for entry in plugin_files.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Load plugin settings
            let plugin_config = path.join("plugin.ron");
            if !plugin_config.exists() || !plugin_config.is_file() {
                log::warn!("Invalid plugin structure: {}", path.as_path().display());
                continue;
            }

            match fs::read_to_string(plugin_config) {
                Ok(file_contents) => match ron::from_str::<PluginConfig>(&file_contents) {
                    Ok(plug) => {
                        let mut plug = plug.clone();
                        plug.path = Some(path.join("main.wasm"));
                        // If any user settings are found, override default ones
                        // from plugin config file.
                        if let Some(user_settings) = config.plugin_settings.get(&plug.name) {
                            for (key, value) in user_settings.iter() {
                                plug.user_settings
                                    .insert(key.to_string(), value.to_string());
                            }
                        }

                        if cmds
                            .send(PluginCommand::Initialize(plug.clone()))
                            .await
                            .is_ok()
                        {
                            log::info!("<{}> plugin found", &plug.name);
                        } else {
                            log::error!("Couldn't send plugin cmd");
                        }
                    }
                    Err(e) => log::error!("Couldn't parse plugin config: {}", e),
                },
                Err(e) => log::error!("Couldn't read plugin config: {}", e),
            }
        }
    }
}

pub fn plugin_init(state: &AppState, plugin: &PluginConfig) -> anyhow::Result<Instance> {
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
    let mut wasi_env = WasiState::new(&plugin.name)
        // Attach the plugin data directory
        .map_dir("/data", plugin.data_folder())
        .expect("Unable to mount plugin data folder")
        // Load user settings as environment variables
        .envs(user_settings.iter())
        // Override stdin/out with pipes for comms
        .stdin(Box::new(input))
        .stdout(Box::new(output))
        .finalize()?;

    let mut import_object = wasi_env.import_object(&module)?;
    // Register exported functions
    import_object.register(
        "spyglass",
        exports::register_exports(state, plugin, &store, &wasi_env),
    );

    // Instantiate the module wn the imports
    let instance = Instance::new(&module, &import_object)?;

    // Lets call the `_start` function, which is our `main` function in Rust
    let start = instance.exports.get_function("_start")?;
    start.call(&[])?;

    Ok(instance)
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

#[allow(dead_code)]
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

#[allow(dead_code)]
fn wasi_read<T: DeserializeOwned>(env: &WasiEnv) -> anyhow::Result<T> {
    let buf = wasi_read_string(env)?;
    Ok(ron::from_str(&buf)?)
}

#[allow(dead_code)]
fn wasi_write(env: &WasiEnv, obj: &(impl Serialize + ?Sized)) -> anyhow::Result<()> {
    wasi_write_string(env, &ron::to_string(&obj)?)
}
