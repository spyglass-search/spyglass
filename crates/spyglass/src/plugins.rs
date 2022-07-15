use std::path::PathBuf;

use serde::Serialize;
use tokio::sync::{broadcast, mpsc};
use wasmer::{Instance, Module, Store};
use wasmer_wasi::WasiState;

use crate::task::AppShutdown;

#[derive(Serialize)]
pub enum PluginType {
    // - Registers itself as a lens.
    // - Enqueues URLs to the crawl queue.
    // - Register to handle specific URLs.
    Lens,
}

#[derive(Serialize)]
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

pub enum PluginResponse {
    Success,
    Error,
}

/// Manages plugin events
#[tracing::instrument(skip_all)]
pub async fn plugin_manager(
    cmd_writer: mpsc::Sender<PluginCommand>,
    mut cmd_queue: mpsc::Receiver<PluginCommand>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    log::info!("plugin manager started");
    plugin_load(cmd_writer).await;

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
                log::info!("intializing plugin <{}>", plugin.name);
                if let Err(e) = plugin_init(&plugin) {
                    log::error!("Unable to init plugin <{}>: {}", plugin.name, e);
                } else {
                    log::info!("plugin <{}> initialized", plugin.name);
                }
            }
            // Nothing to do
            _ => tokio::time::sleep(tokio::time::Duration::from_secs(1)).await,
        }
    }
}

pub async fn plugin_load(cmds: mpsc::Sender<PluginCommand>) {
    let _ = cmds
        .send(PluginCommand::Initialize(PluginData {
            name: "hello-world".into(),
            path: Some("assets/plugins/chrome-importer/main.wasm".into()),
            plugin_type: PluginType::Lens,
        }))
        .await;
}

pub fn plugin_init(plugin: &PluginData) -> anyhow::Result<()> {
    if plugin.path.is_none() {
        // Nothing to do if theres no WASM file to load.
        return Err(anyhow::Error::msg("Unable to find plugin path"));
    }

    let path = plugin.path.as_ref().expect("Unable to extract plugin path");
    let store = Store::default();
    let module = Module::from_file(&store, &path)?;

    // Create the `WasiEnv`
    let mut wasi_env = WasiState::new(&plugin.name).args(&["Gordon"]).finalize()?;

    // Generate an `ImportObject`
    let import_object = wasi_env.import_object(&module)?;

    // Insantiate the module wn the imports
    let instance = Instance::new(&module, &import_object)?;

    // Lets call the `_start` function, which is our `main` function in Rust
    let start = instance.exports.get_function("_start")?;
    start.call(&[])?;

    Ok(())
}
