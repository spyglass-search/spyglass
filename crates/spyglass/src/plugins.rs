use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use shared::config::Config;
use tokio::sync::{broadcast, mpsc};
use wasmer::{Instance, Module, Store, WasmerEnv, Function, Exports};
use wasmer_wasi::WasiState;

use crate::state::AppState;
use crate::task::AppShutdown;

#[derive(Clone, Deserialize, Serialize)]
pub enum PluginType {
    // - Registers itself as a lens.
    // - Enqueues URLs to the crawl queue.
    // - Register to handle specific URLs.
    Lens,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PluginData {
    pub name: String,
    #[serde(default)]
    pub path: Option<PathBuf>,
    pub plugin_type: PluginType,
}

pub enum PluginCommand {
    Initialize(PluginData),
    Queue,
}

// Basic environment information for the plugin
#[derive(WasmerEnv, Clone)]
pub (crate) struct PluginEnv {
    plugin_id: u32,
}

/// Manages plugin events
#[tracing::instrument(skip_all)]
pub async fn plugin_manager(
    _state: AppState,
    config: Config,
    cmd_writer: mpsc::Sender<PluginCommand>,
    mut cmd_queue: mpsc::Receiver<PluginCommand>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    log::info!("plugin manager started");
    // Initial load, send some basic configuration to the plugins
    plugin_load(config, cmd_writer).await;

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
            Some(PluginCommand::Initialize(plugin)) => {
                if let Err(e) = plugin_init(&plugin) {
                    log::error!("Unable to init plugin <{}>: {}", plugin.name, e);
                }
            }
            // Nothing to do
            _ => tokio::time::sleep(tokio::time::Duration::from_secs(1)).await,
        }
    }
}

pub async fn plugin_load(config: Config, cmds: mpsc::Sender<PluginCommand>) {
    log::info!("🔌 loading plugins");

    let plugins_dir = config.plugins_dir();
    let plugin_files = fs::read_dir(plugins_dir).expect("Invalid plugin directory");

    for entry in plugin_files.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Load plugin info
            let plugin_config = path.join("plugin.ron");
            if !plugin_config.exists() || !plugin_config.is_file() {
                log::warn!("Invalid plugin structure: {}", path.as_path().display());
                continue;
            }

            match fs::read_to_string(plugin_config) {
                Ok(file_contents) => match ron::from_str::<PluginData>(&file_contents) {
                    Ok(config) => {
                        let mut config = config.clone();
                        config.path = Some(path.join("main.wasm"));
                        if cmds
                            .send(PluginCommand::Initialize(config.clone()))
                            .await
                            .is_ok()
                        {
                            log::info!("<{}> plugin found", &config.name);
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

pub fn plugin_init(plugin: &PluginData) -> anyhow::Result<()> {
    if plugin.path.is_none() {
        // Nothing to do if theres no WASM file to load.
        return Err(anyhow::Error::msg(format!(
            "Unable to find plugin path: {:?}",
            plugin.path
        )));
    }

    let path = plugin.path.as_ref().expect("Unable to extract plugin path");
    let store = Store::default();
    let module = Module::from_file(&store, &path)?;

    // Create the `WasiEnv`
    let log_func = Function::new_native_with_env(
        &store,
        PluginEnv { plugin_id: 1 },
        plugin_log
    );

    let mut env = Exports::new();
    env.insert("plugin_log", log_func);

    let mut wasi_env = WasiState::new(&plugin.name)
        .finalize()?;

    let mut import_object = wasi_env.import_object(&module)?;
    import_object.register("spyglass", env);

    // Insantiate the module wn the imports
    let instance = Instance::new(&module, &import_object)?;

    // Lets call the `_start` function, which is our `main` function in Rust
    let start = instance.exports.get_function("_start")?;
    start.call(&[])?;

    let sum = instance.exports.get_native_function::<(i32, i32), i32>("sum")?;
    let result: i32 = sum.call(3, 4)?;
    log::info!("exported func call: {}", result);

    Ok(())
}

#[tracing::instrument(skip_all)]
fn plugin_log(env: &PluginEnv) {
    log::info!("{}: {}", env.plugin_id, " log called");
}