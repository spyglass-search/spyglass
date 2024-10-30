use crate::crawler::CrawlResult;
use crate::documents;
use entities::models::indexed_document;
use entities::models::tag;
use entities::models::tag::TagPair;
use entities::models::tag::TagType;
use entities::sea_orm::ColumnTrait;
use entities::sea_orm::EntityTrait;
use entities::sea_orm::ModelTrait;
use entities::sea_orm::QueryFilter;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use spyglass_searcher::{RetrievedDocument, WriteTrait};
use std::path::Path;
use std::str::FromStr;
use tokio::sync::mpsc::Sender;
use url::Url;
use wasmer::{Exports, Function, Store};
use wasmer_wasi::WasiEnv;

use super::{wasi_read, wasi_read_string, PluginCommand, PluginConfig, PluginEnv, PluginId};
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
            let docs = indexed_document::Entity::find()
                .filter(indexed_document::Column::Url.eq(url))
                .all(&env.app_state.db)
                .await
                .unwrap_or_default();

            let doc_ids = docs.iter().map(|x| x.doc_id.to_owned()).collect::<Vec<_>>();
            env.app_state.index.delete_many_by_id(&doc_ids).await?;
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
        PluginCommandRequest::HttpRequest {
            headers,
            method,
            url,
            body,
            auth,
        } => {
            let client = reqwest::Client::new();
            let header_map = build_headermap(headers, &env.name);
            let method_type = convert_method(method);

            let request = client.request(method_type, url).headers(header_map);
            let request = if let Some(body) = body {
                request.body(body.clone())
            } else {
                request
            };

            let request = if let Some(auth) = auth {
                match auth {
                    Authentication::BASIC(key, val) => request.basic_auth(key.clone(), val.clone()),
                    Authentication::BEARER(key) => request.bearer_auth(key.clone()),
                }
            } else {
                request
            };

            let result = request.send().await;

            match result {
                Ok(response) => {
                    let headers = convert_headers(response.headers());
                    let txt = response.text().await.ok();

                    let http_response = spyglass_plugin::HttpResponse {
                        response: txt,
                        headers,
                    };

                    env.cmd_writer
                        .send(PluginCommand::HandleUpdate {
                            plugin_id: env.id,
                            event: PluginEvent::HttpResponse {
                                url: url.clone(),
                                result: Ok(http_response),
                            },
                        })
                        .await?;
                }
                Err(error) => {
                    env.cmd_writer
                        .send(PluginCommand::HandleUpdate {
                            plugin_id: env.id,
                            event: PluginEvent::HttpResponse {
                                url: url.clone(),
                                result: Err(format!("{}", error)),
                            },
                        })
                        .await?;
                }
            }
        }
        PluginCommandRequest::ModifyTags {
            documents,
            tag_modifications,
        } => {
            log::trace!("Received modify tags command {:?}", documents);
            let tag_ids = documents.has_tags.clone().unwrap_or_default();
            let tag_ids = tag::get_tags_by_value(&env.app_state.db, &tag_ids)
                .await
                .unwrap_or_default()
                .iter()
                .map(|model| model.id as u64)
                .collect::<Vec<u64>>();

            let exclude_tags = documents.exclude_tags.clone().unwrap_or_default();
            let exclude_tags = tag::get_tags_by_value(&env.app_state.db, &exclude_tags)
                .await
                .unwrap_or_default()
                .iter()
                .map(|model| model.id as u64)
                .collect::<Vec<u64>>();

            let docs = env
                .app_state
                .index
                .search_by_query(
                    documents.urls.clone(),
                    documents.ids.clone(),
                    &tag_ids,
                    &exclude_tags,
                )
                .await;

            if !docs.is_empty() {
                let docs = docs
                    .iter()
                    .map(|(_, doc)| doc)
                    .cloned()
                    .collect::<Vec<RetrievedDocument>>();

                if let Err(error) =
                    documents::update_tags(&env.app_state, &docs, tag_modifications).await
                {
                    log::error!("Error updating document tags {:?}", error);
                }
            }
        }
        PluginCommandRequest::AddDocuments { documents, tags } => {
            if !documents.is_empty() {
                let (crawl_results, tags) = convert_docs_to_crawl(documents, tags);

                if let Err(error) =
                    documents::process_crawl_results(&env.app_state, &crawl_results, &tags).await
                {
                    log::error!("Error updating documents {:?}", error);
                }
            }
        }
        PluginCommandRequest::SubscribeForUpdates => {
            env.cmd_writer
                .send(PluginCommand::SubscribeForUpdates(env.id))
                .await?;
        }
    }

    Ok(())
}

// Converts local method enum and http method enum
fn convert_method(method: &HttpMethod) -> reqwest::Method {
    match method {
        HttpMethod::GET => reqwest::Method::GET,
        HttpMethod::DELETE => reqwest::Method::DELETE,
        HttpMethod::POST => reqwest::Method::POST,
        HttpMethod::PUT => reqwest::Method::PUT,
        HttpMethod::PATCH => reqwest::Method::PATCH,
        HttpMethod::HEAD => reqwest::Method::HEAD,
        HttpMethod::OPTIONS => reqwest::Method::OPTIONS,
    }
}

// Helper method used to convert header list to header map
fn build_headermap(headers: &Vec<(String, String)>, plugin: &str) -> HeaderMap {
    let mut header_map = HeaderMap::new();

    header_map.append(
        reqwest::header::USER_AGENT,
        format!("Spyglass-Plugin-{plugin}").parse().unwrap(),
    );
    for (key, val) in headers {
        let header_val = HeaderValue::from_str(val);
        let header_name = HeaderName::from_str(key);
        if let (Ok(header_val), Ok(header_name)) = (header_val, header_name) {
            header_map.append(header_name, header_val);
        }
    }
    header_map
}

// Converts header map to header list
fn convert_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .into_iter()
        .map(|(key, val)| (key.to_string(), val.to_str().unwrap_or("").to_owned()))
        .collect::<Vec<(String, String)>>()
}

fn convert_docs_to_crawl(
    documents: &[DocumentUpdate],
    tags: &[(String, String)],
) -> (Vec<CrawlResult>, Vec<TagPair>) {
    let crawls = documents
        .iter()
        .filter_map(|doc| {
            let tags = convert_tags(&doc.tags);
            match Url::parse(doc.url.as_str()) {
                Ok(url) => {
                    let mut crawl = CrawlResult::new(
                        &url,
                        doc.open_url.clone(),
                        doc.content.clone().unwrap_or(String::from("")).as_str(),
                        doc.title.clone().unwrap_or(String::from("")).as_str(),
                        doc.description.clone(),
                    );
                    crawl.tags = tags;
                    Some(crawl)
                }
                Err(error) => {
                    log::error!("Invalid url specified for document {:?}", error);
                    None
                }
            }
        })
        .collect::<Vec<CrawlResult>>();

    let converted_tags = convert_tags(tags);
    (crawls, converted_tags)
}

fn convert_tags(tags: &[(String, String)]) -> Vec<TagPair> {
    tags.iter()
        .map(|(key, val)| {
            let tag_type = TagType::string_to_tag_type(key);
            (tag_type, val.clone())
        })
        .collect::<Vec<TagPair>>()
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
    let tag_ids = query.has_tags.clone().unwrap_or_default();
    let tag_ids = tag::get_tags_by_value(&env.app_state.db, &tag_ids)
        .await
        .unwrap_or_default()
        .iter()
        .map(|model| model.id as u64)
        .collect::<Vec<u64>>();

    let exclude_tags = query.exclude_tags.clone().unwrap_or_default();
    let exclude_tags = tag::get_tags_by_value(&env.app_state.db, &exclude_tags)
        .await
        .unwrap_or_default()
        .iter()
        .map(|model| model.id as u64)
        .collect::<Vec<u64>>();

    let docs = env
        .app_state
        .index
        .search_by_query(
            query.urls.clone(),
            query.ids.clone(),
            &tag_ids,
            &exclude_tags,
        )
        .await;
    log::debug!("Found {:?} documents for query", docs.len());
    let mut results = Vec::new();
    let db = &env.app_state.db;
    for (_score, doc) in docs {
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

fn handle_plugin_enqueue(env: &PluginEnv, urls: &[String]) {
    log::info!("{} enqueuing {} urls", env.name, urls.len());
    let state = env.app_state.clone();
    // Grab a handle to the plugin manager runtime
    let rt = tokio::runtime::Handle::current();
    let urls = urls.to_owned();

    rt.spawn(async move {
        let state = state.clone();
        if let Err(e) = enqueue_all(
            &state.db.clone(),
            &urls,
            &[],
            &state.user_settings.load(),
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
