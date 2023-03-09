use crate::documents;
use entities::models::indexed_document;
use entities::models::tag;
use serde::{Deserialize, Serialize};
use spyglass_plugin::{DocumentResult, PluginEvent};
use std::path::Path;
use tantivy::DocAddress;
use tokio::sync::mpsc::Sender;
use wasmer::{Exports, Function, Store};
use wasmer_wasi::WasiEnv;

use entities::sea_orm::ColumnTrait;
use entities::sea_orm::EntityTrait;
use entities::sea_orm::ModelTrait;
use entities::sea_orm::QueryFilter;

use super::{wasi_read, wasi_read_string, PluginCommand, PluginConfig, PluginEnv, PluginId};
use crate::search::{self, Searcher};
use crate::state::AppState;

use entities::models::crawl_queue::{enqueue_all, EnqueueSettings};
use spyglass_plugin::{DocumentQuery, PluginCommandRequest};

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
        _data_dir: plugin.data_folder(),
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
        PluginCommandRequest::QueryDocuments { query, subscribe } => {
            if *subscribe {
                tokio::spawn(query_document_and_send_loop(env.clone(), query.clone()));
            } else {
                query_documents_and_send(env, query, true).await;
            }
        }
        PluginCommandRequest::ModifyTags {
            documents,
            tag_modifications,
        } => {
            log::trace!("Received modify tags command {:?}", documents);
            let docs =
                Searcher::search_by_query(&env.app_state.db, &env.app_state.index, documents).await;
            if !docs.is_empty() {
                let doc_ids = docs
                    .iter()
                    .map(|(_, addr)| *addr)
                    .collect::<Vec<DocAddress>>();
                if let Err(error) =
                    documents::update_tags(&env.app_state, &doc_ids, tag_modifications).await
                {
                    log::error!("Error updating document tags {:?}", error);
                }
            }
        }
    }

    Ok(())
}

async fn query_document_and_send_loop(env: PluginEnv, query: DocumentQuery) {
    let mut timer = tokio::time::interval(tokio::time::Duration::from_secs(60));
    loop {
        timer.tick().await;
        {
            let manager = &env.app_state.plugin_manager.lock().await;
            if !manager.is_enabled(env.id) {
                log::debug!("Plugin has been disabled removing subscription");
                break;
            }
        }
        query_documents_and_send(&env, &query, false).await;
    }
}

async fn query_documents_and_send(env: &PluginEnv, query: &DocumentQuery, send_empty: bool) {
    let docs = Searcher::search_by_query(&env.app_state.db, &env.app_state.index, query).await;
    log::debug!("Found {:?} documents for query", docs.len());
    let searcher = &env.app_state.index.reader.searcher();
    let mut results = Vec::new();
    let db = &env.app_state.db;
    for (_score, doc_addr) in docs {
        if let Ok(Ok(doc)) = searcher
            .doc(doc_addr)
            .map(|doc| search::document_to_struct(&doc))
        {
            log::trace!("Got id with url {} {}", doc.doc_id, doc.url);
            let indexed = indexed_document::Entity::find()
                .filter(indexed_document::Column::DocId.eq(doc.doc_id.clone()))
                .one(db)
                .await;

            let crawl_uri = doc.url;
            if let Ok(Some(indexed)) = indexed {
                let tags = indexed
                    .find_related(tag::Entity)
                    .all(db)
                    .await
                    .unwrap_or_default()
                    .iter()
                    .map(|tag| (tag.label.to_string(), tag.value.clone()))
                    .collect::<Vec<(String, String)>>();

                let result = DocumentResult {
                    doc_id: doc.doc_id.clone(),
                    domain: doc.domain,
                    title: doc.title,
                    description: doc.description,
                    url: indexed.open_url.unwrap_or(crawl_uri),
                    tags,
                };

                results.push(result);
            }
        }
    }

    if !results.is_empty() || send_empty {
        let _ = env
            .cmd_writer
            .send(PluginCommand::HandleUpdate {
                plugin_id: env.id,
                event: PluginEvent::DocumentResponse {
                    request_id: String::from("ahhh_my_id"),
                    page_count: 1,
                    page: 0,
                    documents: results,
                },
            })
            .await;
    }
}

/// Handle plugin calls into the host environment. These are run as separate tokio tasks
/// so we don't block the main thread.
pub(crate) fn plugin_cmd(env: &PluginEnv) {
    log::debug!("Plugin Command Request Received");
    match wasi_read::<PluginCommandRequest>(&env.wasi_env) {
        Ok(cmd) => {
            // Handle the plugin command as a separate async task
            let rt = tokio::runtime::Handle::current();

            #[cfg(feature = "tokio-console")]
            tokio::task::Builder::new()
                .name("Plugin Request")
                .spawn_on(handle_plugin_cmd(cmd, env.clone()), &rt);

            #[cfg(not(feature = "tokio-console"))]
            rt.spawn(handle_plugin_cmd(cmd, env.clone()));
        }
        Err(error) => {
            log::error!("Invalid command request received {:?}", error);
        }
    }
}

// Helper method used to handle the plugin command
async fn handle_plugin_cmd(cmd: PluginCommandRequest, env: PluginEnv) {
    if let Err(e) = handle_plugin_cmd_request(&cmd, &env).await {
        log::error!(
            "Could not handle cmd {:?} for plugin {}. Error: {}",
            cmd,
            env.name,
            e
        );
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
fn _handle_sync_file(env: &PluginEnv, dst: &str, src: &str) {
    log::info!("<{}> requesting access to file: {}", env.name, src);
    let dst = Path::new(dst.trim_start_matches('/'));
    let src = Path::new(&src);

    if let Some(file_name) = src.file_name() {
        let dst = env._data_dir.join(dst).join(file_name);
        // Attempt to copy file into plugin data directory
        if let Err(e) = std::fs::copy(src, dst) {
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
