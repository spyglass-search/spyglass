use anyhow::Error;
use ignore::WalkBuilder;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::Sender;
use wasmer::{Exports, Function, Store};
use wasmer_wasi::WasiEnv;

use entities::models::crawl_queue::{enqueue_all, EnqueueSettings};
use spyglass_plugin::{ListDirEntry, PluginCommandRequest};

use super::{
    wasi_read, wasi_read_string, wasi_write, PluginCommand, PluginConfig, PluginEnv, PluginId,
};
use crate::search::Searcher;
use crate::state::AppState;

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

async fn handle_plugin_cmd_request(
    cmd: &PluginCommandRequest,
    env: &PluginEnv,
) -> anyhow::Result<()> {
    match cmd {
        // Delete document from index
        PluginCommandRequest::DeleteDoc { url } => {
            Searcher::delete_by_url(&env.app_state, url).await?
        }
        // Enqueue a list of URLs to be crawled
        PluginCommandRequest::Enqueue { urls } => handle_plugin_enqueue(env, urls),
        PluginCommandRequest::ListDir { path } => {
            log::info!("{} ListDir path: {}", env.name, path);
            let entries = std::fs::read_dir(path)?
                .flatten()
                .map(|entry| {
                    let path = entry.path();
                    ListDirEntry {
                        path: path.display().to_string(),
                        is_file: path.is_file(),
                        is_dir: path.is_dir(),
                    }
                })
                .collect::<Vec<ListDirEntry>>();
            wasi_write(&env.wasi_env, &entries)?;
        }
        // Subscribe to a plugin event
        PluginCommandRequest::Subscribe(event) => {
            env.cmd_writer
                .send(PluginCommand::Subscribe(env.id, event.clone()))
                .await?;
            log::info!("<{}> subscribed to {}", env.name.clone(), event);
        }
        // NOTE: This is a hack since sqlite can't easily be compiled into WASM yet.
        // This is for plugins who need to run a query against some sqlite3 file,
        // for example the Firefox bookmarks/history are store in such a file.
        PluginCommandRequest::SqliteQuery { path, query } => {
            let db_path = env.data_dir.join(path);
            if !db_path.exists() {
                return Err(Error::msg(format!("Invalid sqlite db path: {}", path)));
            }

            let conn = Connection::open(db_path)?;
            let mut stmt = conn.prepare(query)?;

            let results = stmt.query_map([], |row| {
                Ok(row.get::<usize, String>(0).unwrap_or_default())
            })?;

            let collected: Vec<String> = results
                .map(|x| x.unwrap_or_default())
                .collect::<Vec<String>>()
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect();

            wasi_write(&env.wasi_env, &collected)?;
        }
        PluginCommandRequest::SyncFile { dst, src } => handle_sync_file(env, dst, src),
        // Walk through a path & enqueue matching files for indexing.
        PluginCommandRequest::WalkAndEnqueue { path, extensions } => {
            let dir_path = Path::new(&path);
            if !dir_path.exists() {
                return Err(Error::msg(format!("Invalid path: {}", path)));
            }

            log::info!("{} crawling path: {}", env.name, path);
            let stats = handle_walk_and_enqueue(dir_path.to_path_buf(), extensions);
            wasi_write(&env.wasi_env, &stats)?;
        }
    }

    Ok(())
}

/// Handle plugin calls into the host environment. These are run as separate tokio tasks
/// so we don't block the main thread.
pub(crate) fn plugin_cmd(env: &PluginEnv) {
    if let Ok(cmd) = wasi_read::<PluginCommandRequest>(&env.wasi_env) {
        // Handle the plugin command as a separate async task
        let rt = tokio::runtime::Handle::current();
        let env = env.clone();
        rt.spawn(async move {
            if let Err(e) = handle_plugin_cmd_request(&cmd, &env).await {
                log::error!(
                    "Could not handle cmd {:?} for plugin {}. Error: {}",
                    cmd,
                    env.name,
                    e
                );
            }
        });
    }
}

/// Log call from the plugin. This is a utility function since the plugin has
/// has direct stdio/stdout access.
pub(crate) fn plugin_log(env: &PluginEnv) {
    if let Ok(msg) = wasi_read_string(&env.wasi_env) {
        log::info!("{}: {}", env.name, msg);
    }
}

/// Adds a file into the plugin data directory. Use this to copy files from elsewhere
/// in the filesystem so that it can be processed by the plugin.
fn handle_sync_file(env: &PluginEnv, dst: &str, src: &str) {
    log::info!("<{}> requesting access to folder: {}", env.name, src);
    let dst = Path::new(dst.trim_start_matches('/'));
    let src = Path::new(&src);

    if let Some(file_name) = src.file_name() {
        let dst = env.data_dir.join(dst).join(file_name);
        // Attempt to copy file into plugin data directory
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
            &EnqueueSettings {
                force_allow: true,
                ..Default::default()
            },
            Option::None,
        )
        .await
        {
            log::error!("error adding to queue: {}", e);
        }
    });
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct WalkStats {
    pub dirs: i32,
    pub files: i32,
    pub skipped: i32,
}

fn handle_walk_and_enqueue(path: PathBuf, extensions: &[String]) -> WalkStats {
    let exts: HashSet<String> = extensions
        .iter()
        .map(|x| x.to_owned())
        .collect::<HashSet<String>>();

    let walker = WalkBuilder::new(path).standard_filters(true).build();

    let mut stats = WalkStats::default();
    for entry in walker.flatten() {
        if let Some(file_type) = entry.file_type() {
            if file_type.is_dir() {
                stats.dirs += 1;
                continue;
            }

            let ext = entry.path().extension().and_then(|ext| ext.to_str());

            if let Some(ext) = ext {
                if exts.contains(ext) {
                    stats.files += 1;
                } else {
                    stats.skipped += 1;
                }
            }
        }
    }

    stats
}

#[cfg(test)]
mod test {
    use super::handle_list_dir;
    use std::path::Path;

    #[test]
    fn test_list_dir() {
        let ext = vec!["md".into(), "txt".into()];
        let path = Path::new("/Users/a5huynh/Documents");
        let stats = handle_list_dir(path.to_path_buf(), &ext);
        println!("stats: {:?}", stats);
    }
}
