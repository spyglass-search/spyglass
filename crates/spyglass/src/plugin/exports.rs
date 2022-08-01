use rusqlite::Connection;
use std::path::Path;
use tokio::sync::mpsc::Sender;
use wasmer::{Exports, Function, Store};
use wasmer_wasi::WasiEnv;

use super::{
    wasi_read, wasi_read_string, wasi_write, PluginCommand, PluginConfig, PluginEnv, PluginId,
};
use crate::state::AppState;
use entities::models::crawl_queue::enqueue_all;
use spyglass_plugin::PluginCommandRequest;

pub fn register_exports(
    plugin_id: PluginId,
    state: &AppState,
    cmd_writer: &Sender<PluginCommand>,
    plugin: &PluginConfig,
    store: &Store,
    env: &WasiEnv,
) -> Exports {
    let mut exports = Exports::new();
    let env = PluginEnv {
        id: plugin_id,
        name: plugin.name.clone(),
        app_state: state.clone(),
        data_dir: plugin.data_folder(),
        wasi_env: env.clone(),
        cmd_writer: cmd_writer.clone(),
    };

    exports.insert(
        "plugin_cmd",
        Function::new_native_with_env(store, env.clone(), plugin_cmd),
    );
    exports.insert(
        "plugin_log",
        Function::new_native_with_env(store, env, plugin_log),
    );
    exports
}

pub(crate) fn plugin_cmd(env: &PluginEnv) {
    if let Ok(cmd) = wasi_read::<PluginCommandRequest>(&env.wasi_env) {
        match cmd {
            PluginCommandRequest::Enqueue { urls } => handle_plugin_enqueue(env, &urls),
            PluginCommandRequest::ListDir(path) => {
                let entries = if let Ok(entries) = std::fs::read_dir(path) {
                    entries
                        .flatten()
                        .map(|entry| entry.path().display().to_string())
                        .collect::<Vec<String>>()
                } else {
                    Vec::new()
                };

                if let Err(e) = wasi_write(&env.wasi_env, &entries) {
                    log::error!("<{}> unable to list dir: {}", env.id, e);
                }
            }
            PluginCommandRequest::Subscribe(event) => {
                let writer = env.cmd_writer.clone();
                let plugin_id = env.id;

                let rt = tokio::runtime::Handle::current();
                rt.spawn(async move {
                    let writer = writer.clone();
                    if let Err(e) = writer
                        .send(PluginCommand::Subscribe(plugin_id, event))
                        .await
                    {
                        log::error!("Unable to subscribe plugin <{}> to event: {}", plugin_id, e);
                    }
                });
            }
            PluginCommandRequest::SqliteQuery { path, query } => {
                let path = env.data_dir.join(path);
                if let Ok(conn) = Connection::open(path) {
                    let stmt = conn.prepare(&query);
                    if let Ok(mut stmt) = stmt {
                        let results = stmt.query_map([], |row| {
                            Ok(row.get::<usize, String>(0).unwrap_or_default())
                        });

                        if let Ok(results) = results {
                            let collected: Vec<String> = results
                                .map(|x| x.unwrap_or_default())
                                .collect::<Vec<String>>()
                                .into_iter()
                                .filter(|x| !x.is_empty())
                                .collect();

                            if let Err(e) = wasi_write(&env.wasi_env, &collected) {
                                log::error!("{}", e);
                            }
                        }
                    }
                }
            }
            PluginCommandRequest::SyncFile { dst, src } => handle_sync_file(env, &dst, &src),
        }
    }
}

pub(crate) fn plugin_log(env: &PluginEnv) {
    if let Ok(msg) = wasi_read_string(&env.wasi_env) {
        log::info!("{}: {}", env.name, msg);
    }
}

/// Adds a file into the plugin data directory. Use this to copy files from elsewhere
/// in the filesystem so that it can be processed by the plugin.
pub(crate) fn handle_sync_file(env: &PluginEnv, dst: &str, src: &str) {
    log::info!("<{}> requesting access to folder: {}", env.name, src);

    let dst = Path::new(&dst)
        .strip_prefix("/data")
        .unwrap_or_else(|_| Path::new(""));

    let src = Path::new(&src);
    if let Some(file_name) = src.file_name() {
        let dst = env.data_dir.join(dst).join(file_name);
        // Attempt to mount directory
        if let Err(e) = std::fs::copy(src, &dst) {
            log::error!("Unable to copy into plugin data dir: {}", e);
        }
    } else {
        log::error!("Source must be a file: {}", src.display());
    }
}

fn handle_plugin_enqueue(env: &PluginEnv, urls: &Vec<String>) {
    log::info!("{} enqueuing {} urls", env.name, urls.len());
    let state = env.app_state.clone();
    // Grab a handle to the plugin manager runtime
    let rt = tokio::runtime::Handle::current();
    let urls = urls.clone();
    rt.spawn(async move {
        let state = state.clone();
        if let Err(e) = enqueue_all(
            &state.db.clone(),
            &urls,
            &[],
            &state.user_settings,
            &Default::default(),
        )
        .await
        {
            log::error!("error adding to queue: {}", e);
        }
    });
}
