use super::response;
use directories::UserDirs;
use entities::get_library_stats;
use entities::models::crawl_queue::{CrawlStatus, EnqueueSettings};
use entities::models::lens::LensType;
use entities::models::tag::TagType;
use entities::models::{
    bootstrap_queue, connection::get_all_connections, crawl_queue, fetch_history, indexed_document,
    lens,
};
use entities::sea_orm::{prelude::*, sea_query};
use jsonrpsee::core::RpcResult;
use libnetrunner::parser::html::html_to_text;
use libspyglass::connection::{self, credentials, handle_authorize_connection};
use libspyglass::crawler::CrawlResult;
use libspyglass::documents::process_crawl_results;
use libspyglass::filesystem;
use libspyglass::state::AppState;
use libspyglass::task::{AppPause, UserSettingsChange};
use num_format::{Locale, ToFormattedString};
use shared::config::{self, Config, UserSettings};
use shared::llm::{ChatMessage, ChatStream, LlmSession};
use shared::metrics::Event;
use shared::request::{BatchDocumentRequest, RawDocType, RawDocumentRequest};
use shared::response::{
    AppStatus, DefaultIndices, InstallStatus, LensResult, LibraryStats, ListConnectionResult,
    PluginResult, SupportedConnection, UserConnection,
};
use spyglass_llm::LlmClient;
use spyglass_rpc::{server_error, RpcEvent, RpcEventType};
use spyglass_searcher::WriteTrait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use tracing::instrument;
use url::Url;

pub mod search;

pub async fn add_document_batch(state: &AppState, req: &BatchDocumentRequest) -> RpcResult<()> {
    // Validate tags and consolidate tags
    let mut tags = Vec::new();
    tags.push((TagType::Source, req.source.to_string()));
    for (tag_type, tag_value) in req.tags.iter() {
        if let Ok(ttype) = TagType::from_str(tag_type) {
            if !tag_value.is_empty() {
                tags.push((ttype, tag_value.to_owned()));
            } else {
                log::warn!("Invalid tag value `{tag_value}` for tag type: {tag_type}");
            }
        } else {
            log::warn!("Invalid tag type: {tag_type}");
        }
    }

    // No need to process anything, simply add to the crawl queue for processing
    log::debug!(
        "Enqueueing {} URLs from webext w/ tags: {:?}",
        req.urls.len(),
        &tags
    );
    let overrides = EnqueueSettings {
        force_allow: true,
        is_recrawl: true,
        tags,
        ..Default::default()
    };

    if let Err(err) = crawl_queue::enqueue_all(
        &state.db,
        &req.urls,
        &[],
        &state.user_settings.load(),
        &overrides,
        None,
    )
    .await
    {
        return Err(server_error(format!("Unable to queue URL: {err}"), None));
    }

    Ok(())
}

/// Adds a raw document to the user's index.
pub async fn add_raw_document(state: &AppState, req: &RawDocumentRequest) -> RpcResult<()> {
    // Validate tags and consolidate tags
    let mut tags = Vec::new();
    tags.push((TagType::Source, req.source.to_string()));
    for (tag_type, tag_value) in req.tags.iter() {
        if let Ok(ttype) = TagType::from_str(tag_type) {
            if !tag_value.is_empty() {
                tags.push((ttype, tag_value.to_owned()));
            } else {
                log::warn!("Invalid tag value `{tag_value}` for tag type: {tag_type}");
            }
        } else {
            log::warn!("Invalid tag type: {tag_type}");
        }
    }

    match req.doc_type {
        RawDocType::Html => {
            // Parse content
            let content = req
                .content
                .as_ref()
                .map(|s| s.to_owned())
                .unwrap_or_default();

            let res = html_to_text(&req.url, &content);
            let url = match res.canonical_url.map(|s| Url::parse(&s)) {
                Some(Ok(url)) => url,
                _ => {
                    return Err(server_error(format!("Invalid URL: {}", req.url), None));
                }
            };

            let mut crawl = CrawlResult::new(
                &url,
                Some(url.to_string()),
                &res.content,
                &res.title.unwrap_or_default(),
                None,
            );

            // Add tags to document
            crawl.tags.extend(tags);

            // Add to index
            log::debug!("adding to index: {} - {:?}", crawl.url, crawl.tags);
            if let Err(err) = process_crawl_results(state, &[crawl], &Vec::new()).await {
                log::error!("Unable to add from webext: {}", err);
            }
        }
        // No need to process anything, we can add this directly to the index.
        RawDocType::Text => {
            log::debug!("RawDocType::Text is not supported yet");
        }
        // No need to process anything, simply add to the crawl queue for processing
        RawDocType::Url => {
            log::debug!("Enqueueing URL from webext: {} - {:?}", req.url, &tags);
            let overrides = EnqueueSettings {
                force_allow: true,
                is_recrawl: true,
                tags,
                ..Default::default()
            };

            if let Err(err) = crawl_queue::enqueue_all(
                &state.db,
                &[req.url.clone()],
                &[],
                &state.user_settings.load(),
                &overrides,
                None,
            )
            .await
            {
                return Err(server_error(format!("Unable to queue URL: {err}"), None));
            }
        }
    }

    Ok(())
}

#[instrument(skip(state))]
pub async fn authorize_connection(state: AppState, api_id: String) -> RpcResult<()> {
    log::debug!("authorizing <{}>", api_id);
    state
        .metrics
        .track(Event::AuthorizeConnection {
            api_id: api_id.clone(),
        })
        .await;

    if let Err(err) = handle_authorize_connection(&state, &api_id).await {
        Err(server_error(
            format!("Unable to authorize {api_id}: {err}"),
            None,
        ))
    } else {
        Ok(())
    }
}

/// Fun stats about index size, etc.
#[instrument(skip(state))]
pub async fn app_status(state: AppState) -> RpcResult<AppStatus> {
    // Grab details about index
    let index = state.index;
    let reader = index.reader.searcher();

    Ok(AppStatus {
        num_docs: reader.num_docs(),
    })
}

/// Remove a doc from the index
#[instrument(skip(state))]
pub async fn delete_document(state: AppState, id: String) -> RpcResult<()> {
    if let Err(e) = state.index.delete(&id).await {
        log::error!("Unable to delete doc {} due to {}", id, e);
        return Err(server_error(e.to_string(), None));
    }

    let _ = indexed_document::delete_many_by_doc_id(&state.db, &[id]).await;
    Ok(())
}

#[instrument(skip(state))]
pub async fn chat_completion(state: AppState, session: &LlmSession) -> RpcResult<ChatMessage> {
    let mut llm = state.llm.lock().await;
    let client = match llm.as_mut() {
        Some(client) => client,
        None => {
            let client =
                LlmClient::new("assets/models/llm/llama3/Llama-3.2-3B-Instruct.Q5_K_M.gguf".into())
                    .map_err(|e| server_error(e.to_string(), None))?;
            *llm = Some(client);
            llm.as_mut().unwrap()
        }
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<ChatStream>(10);
    let state_clone = state.clone();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            state_clone
                .publish_event(&RpcEvent {
                    event_type: RpcEventType::ChatStream,
                    payload: Some(serde_json::to_value(&msg).unwrap()),
                })
                .await;

            if msg == ChatStream::ChatDone {
                log::info!("finished streaming");
                break;
            }
        }
    });

    let _ = client
        .chat(session, Some(tx))
        .await
        .map_err(|e| server_error(e.to_string(), None))?;
    Ok(ChatMessage {
        role: shared::llm::ChatRole::Assistant,
        content: "hello".into(),
    })
}

/// Remove a domain from crawl queue & index
#[instrument(skip(state))]
pub async fn delete_domain(state: AppState, domain: String) -> RpcResult<()> {
    // Remove domain from bootstrap queue
    if let Err(err) =
        bootstrap_queue::dequeue(&state.db, format!("https://{domain}").as_str()).await
    {
        log::error!("Error deleting seed_url {} from DB: {}", &domain, &err);
    }

    // Remove items from crawl queue
    let res = crawl_queue::Entity::delete_many()
        .filter(crawl_queue::Column::Domain.eq(domain.clone()))
        .exec(&state.db)
        .await;

    if let Ok(res) = res {
        log::info!("removed {} items from crawl queue", res.rows_affected);
    }

    // Remove items from index
    let indexed = indexed_document::Entity::find()
        .filter(indexed_document::Column::Domain.eq(domain.clone()))
        .all(&state.db)
        .await;

    if let Ok(indexed) = indexed {
        log::debug!("removing docs from index");
        let indexed_count = indexed.len();

        let doc_ids: Vec<String> = indexed.iter().map(|x| x.doc_id.to_string()).collect();
        let _ = state.index.delete_many_by_id(&doc_ids).await;
        let _ = indexed_document::delete_many_by_doc_id(&state.db, &doc_ids).await;

        log::debug!("removed {} items from index", indexed_count);
    }

    Ok(())
}

#[instrument(skip(state))]
pub async fn list_connections(state: AppState) -> RpcResult<ListConnectionResult> {
    match entities::models::connection::Entity::find()
        .all(&state.db)
        .await
    {
        Ok(enabled) => {
            // TODO: Move this into a config / db table?
            let all_conns = credentials::supported_connections();
            let supported = all_conns
                .values()
                .cloned()
                .collect::<Vec<SupportedConnection>>();

            // Get list of enabled connections
            let user_connections = enabled
                .iter()
                .map(|conn| UserConnection {
                    id: conn.api_id.clone(),
                    account: conn.account.clone(),
                    is_syncing: conn.is_syncing,
                })
                .collect::<Vec<UserConnection>>();

            Ok(ListConnectionResult {
                supported,
                user_connections,
            })
        }
        Err(err) => Err(server_error(err.to_string(), None)),
    }
}

/// List of installed lenses
#[instrument(skip(state))]
pub async fn list_installed_lenses(state: AppState) -> RpcResult<Vec<LensResult>> {
    let stats = get_library_stats(&state.db).await.unwrap_or_default();
    let mut lenses: Vec<LensResult> = state
        .lenses
        .iter()
        .map(|lens| {
            let progress = if let Some(lens_stats) = stats.get(&lens.name) {
                // In the middle of installing the lens if no stats are available.
                if lens_stats.enqueued == 0 && lens_stats.indexed == 0 {
                    InstallStatus::Installing {
                        percent: 100,
                        status: "Installing...".to_string(),
                    }
                } else if lens_stats.enqueued == 0 {
                    InstallStatus::Finished {
                        num_docs: lens_stats.indexed as u32,
                    }
                } else {
                    InstallStatus::Installing {
                        percent: lens_stats.percent_done(),
                        status: lens_stats.status_string(),
                    }
                }
            } else {
                InstallStatus::Installing {
                    percent: 100,
                    status: "Installing...".to_string(),
                }
            };

            LensResult {
                author: lens.author.clone(),
                name: lens.name.clone(),
                label: lens.label(),
                description: lens.description.clone().unwrap_or_else(|| "".into()),
                hash: lens.hash.clone(),
                file_path: Some(lens.file_path.clone()),
                progress,
                lens_type: shared::response::LensType::Lens,
                ..Default::default()
            }
        })
        .collect();

    build_filesystem_information(
        &state,
        &mut lenses,
        &stats
            .get(filesystem::FILES_LENS)
            .map(|x| x.to_owned())
            .unwrap_or_default(),
    )
    .await;
    add_connections_information(&state, &mut lenses, &stats).await;

    lenses.sort_by(|x, y| x.label.to_lowercase().cmp(&y.label.to_lowercase()));

    Ok(lenses)
}

// Helper method used to add a len result for all api connections
async fn add_connections_information(
    state: &AppState,
    lenses: &mut Vec<LensResult>,
    stats: &HashMap<String, LibraryStats>,
) {
    let connections = get_all_connections(&state.db).await;
    for connection in connections {
        let api_id = connection.api_id;
        let lens_name = connection::api_id_to_lens(&api_id);
        if let Some(stats) = lens_name.and_then(|s| stats.get(s)) {
            if let Some((title, description)) = connection::get_api_description(&api_id) {
                let progress = if connection.is_syncing {
                    InstallStatus::Installing {
                        percent: 0,
                        status: format!(
                            "Syncing {} of many...",
                            stats.indexed.to_formatted_string(&Locale::en)
                        ),
                    }
                } else {
                    InstallStatus::Finished {
                        num_docs: stats.indexed as u32,
                    }
                };

                lenses.push(LensResult {
                    author: String::from("spyglass-search"),
                    name: api_id.clone(),
                    label: String::from(title),
                    description: String::from(description),
                    progress,
                    lens_type: shared::response::LensType::API,
                    ..Default::default()
                });
            }
        }
    }
}

// Helper method used to build a len result for the filesystem
async fn build_filesystem_information(
    state: &AppState,
    lenses: &mut Vec<LensResult>,
    stats: &LibraryStats,
) {
    if !filesystem::is_watcher_enabled() {
        return;
    }

    let watcher = state.file_watcher.lock().await;
    if let Some(watcher) = watcher.as_ref() {
        let total_paths = watcher.processed_path_count().await as u32;
        let path = watcher.initializing_path().await;

        let indexed: u32 = stats.indexed as u32;
        let failed: u32 = stats.failed as u32;

        let total_finished = indexed + failed;
        let mut status = InstallStatus::Finished { num_docs: indexed };
        if stats.enqueued > 0 && total_finished < total_paths {
            let percent = (((indexed * 100) / total_paths) as i32).min(100);
            let status_msg = format!(
                "Processing {} of {}",
                indexed.to_formatted_string(&Locale::en),
                total_paths.to_formatted_string(&Locale::en),
            );
            let status_msg = match path {
                Some(path) => format!("{}. Walking {path}.", status_msg),
                None => status_msg,
            };

            status = InstallStatus::Installing {
                percent,
                status: status_msg,
            };
        }

        let res = LensResult {
            author: String::from("spyglass-search"),
            name: String::from("local-file-system"),
            label: String::from("Local File System"),
            description: String::from("All files are processed locally. Contents of supported file types will be indexed. All unsupported files/folders will be indexed based on their path, name, and extension."),
            progress: status,
            lens_type: shared::response::LensType::Internal,
            ..Default::default()
        };
        lenses.push(res);
    }
}

pub async fn list_plugins(state: AppState) -> RpcResult<Vec<PluginResult>> {
    let mut plugins = Vec::new();
    let result = lens::Entity::find()
        .filter(lens::Column::LensType.eq(LensType::Plugin))
        .all(&state.db)
        .await;

    if let Ok(results) = result {
        for plugin in results {
            plugins.push(PluginResult {
                author: plugin.author,
                title: plugin.name,
                description: plugin.description.clone().unwrap_or_default(),
                is_enabled: plugin.is_enabled,
            });
        }
    }

    plugins.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(plugins)
}

/// Show the list of URLs in the queue and their status
#[allow(dead_code)]
#[instrument(skip(state))]
pub async fn list_queue(state: AppState) -> RpcResult<response::ListQueue> {
    let db = &state.db;
    let queue = crawl_queue::Entity::find().all(db).await;

    match queue {
        Ok(queue) => Ok(response::ListQueue { queue }),
        Err(err) => Err(server_error(err.to_string(), None)),
    }
}

#[instrument(skip(state))]
pub async fn recrawl_domain(state: AppState, domain: String) -> RpcResult<()> {
    log::info!("handling recrawl domain: {}", domain);
    let db = &state.db;

    let _ = fetch_history::Entity::delete_many()
        .filter(fetch_history::Column::Domain.eq(domain.clone()))
        .exec(db)
        .await;

    // Handle cases where we incorrectly stored the web.archive.org URL in the fetch_history
    let _ = fetch_history::Entity::delete_many()
        .filter(fetch_history::Column::Path.contains(&domain))
        .exec(db)
        .await;

    let res = crawl_queue::Entity::update_many()
        .col_expr(
            crawl_queue::Column::Status,
            sea_query::Expr::value(CrawlStatus::Queued),
        )
        .filter(crawl_queue::Column::Domain.eq(domain.clone()))
        .exec(db)
        .await;

    // Log out issues
    if let Err(e) = res {
        log::error!("Error recrawling domain {}: {}", domain, e);
    }

    Ok(())
}

#[instrument(skip(state))]
pub async fn toggle_pause(state: AppState, is_paused: bool) -> RpcResult<()> {
    // Scope so that the app_state mutex is correctly released.
    if let Some(sender) = state.pause_cmd_tx.lock().await.as_ref() {
        let _ = sender.send(if is_paused {
            AppPause::Pause
        } else {
            AppPause::Run
        });
    }

    Ok(())
}

#[instrument(skip(app, _config))]
pub async fn update_user_settings(
    app: &AppState,
    _config: &Config,
    user_settings: &UserSettings,
) -> RpcResult<UserSettings> {
    if let Err(error) = app
        .config_cmd_tx
        .lock()
        .await
        .send(UserSettingsChange::SettingsChanged(user_settings.clone()))
    {
        Err(server_error(error.to_string(), None))
    } else {
        Ok(user_settings.clone())
    }
}

#[instrument(skip(app))]
pub async fn user_settings(app: &AppState) -> RpcResult<UserSettings> {
    Ok(app.user_settings.load().as_ref().clone())
}

#[instrument(skip(state))]
pub async fn uninstall_lens(state: AppState, config: &Config, name: &str) -> RpcResult<()> {
    // Remove from filesystem
    let lens_path = config.lenses_dir().join(format!("{name}.ron"));
    let config = state.lenses.remove(name);
    let _ = std::fs::remove_file(lens_path);

    // Remove from database
    // - remove from lens table
    let _ = lens::Entity::delete_many()
        .filter(lens::Column::Name.eq(name))
        .exec(&state.db)
        .await;

    // - find relevant doc ids to remove
    if let Ok(ids) = indexed_document::find_by_lens(state.db.clone(), name).await {
        // - remove from db & index
        let doc_ids: Vec<String> = ids.iter().map(|x| x.doc_id.to_owned()).collect();
        let dbids: Vec<i64> = ids.iter().map(|x| x.id).collect();

        // Remove from index
        if let Err(err) = state.index.delete_many_by_id(&doc_ids).await {
            return Err(server_error(err.to_string(), None));
        }
        // Remove from db
        let _ = indexed_document::delete_many_by_id(&state.db, &dbids).await;
    }

    // -- remove from crawl queue
    if let Err(err) = crawl_queue::delete_by_lens(state.db.clone(), name).await {
        return Err(server_error(err.to_string(), None));
    }

    // - remove seed urls from bootstrap queue table
    if let Some((_, config)) = config {
        let _ = bootstrap_queue::dequeue(&state.db, &config.name).await;
    }

    state
        .publish_event(&RpcEvent {
            event_type: RpcEventType::LensUninstalled,
            payload: Some(serde_json::to_value(name.to_string()).unwrap()),
        })
        .await;

    Ok(())
}

pub async fn default_indices() -> DefaultIndices {
    let mut file_paths: Vec<PathBuf> = Vec::new();

    if let Some(user_dirs) = UserDirs::new() {
        if let Some(path) = user_dirs.desktop_dir() {
            file_paths.push(path.to_path_buf());
        }

        if let Some(path) = user_dirs.document_dir() {
            file_paths.push(path.to_path_buf());
        }
    }

    // Application path is os dependent
    // NOTE: Uncomment when we add in app searching ability
    // if cfg!(target_os = "macos") {
    //     file_paths.push("/Applications".into());
    // } else if cfg!(target_os = "windows") {
    //     file_paths.push("C:\\Program Files (x86)".into());
    // }

    file_paths.retain(|f| f.exists());
    DefaultIndices {
        file_paths,
        extensions: config::DEFAULT_EXTENSIONS
            .iter()
            .map(|val| String::from(*val))
            .collect(),
    }
}

#[cfg(test)]
mod test {
    use super::uninstall_lens;
    use entities::models::tag::TagType;
    use entities::sea_orm::{ActiveModelTrait, EntityTrait, Set};
    use entities::{
        models::{crawl_queue, indexed_document},
        test::setup_test_db,
    };
    use libspyglass::state::AppState;
    use shared::config::{Config, LensConfig};
    use spyglass_searcher::schema::{DocumentUpdate, ToDocument};
    use spyglass_searcher::WriteTrait;

    #[tokio::test]
    async fn test_uninstall_lens() {
        let db = setup_test_db().await;
        let state = AppState::builder().with_db(db.clone()).build();

        let mut config = Config::new();
        let lens = LensConfig {
            name: "test".to_string(),
            urls: vec!["https://example.com".into()],
            ..Default::default()
        };

        state
            .index
            .upsert(
                &DocumentUpdate {
                    doc_id: Some("test_id".into()),
                    title: "test title",
                    domain: "example.com",
                    url: "https://example.com/test",
                    content: "test content",
                    tags: &[],
                    published_at: None,
                    last_modified: None,
                }
                .to_document(),
            )
            .await
            .expect("Unable to add doc");
        let _ = state.index.save().await;

        let doc = indexed_document::ActiveModel {
            domain: Set("example.com".into()),
            url: Set("https://example.com/test".into()),
            doc_id: Set("test_id".into()),
            ..Default::default()
        };

        let model = doc.insert(&db).await.expect("Unable to insert doc");
        model
            .insert_tags(&db, &[(TagType::Lens, lens.name.clone())])
            .await
            .expect("Unable to insert tags");

        config.lenses.insert(lens.name.clone(), lens.clone());
        uninstall_lens(state.clone(), &config, &lens.name)
            .await
            .expect("Unable to uninstall");

        let cqs = crawl_queue::Entity::find()
            .all(&state.db)
            .await
            .expect("Unable to find crawl tasks");
        assert_eq!(cqs.len(), 0);

        let indexed = indexed_document::Entity::find()
            .all(&state.db)
            .await
            .expect("Unable to find indexed docs");
        assert_eq!(indexed.len(), 0);
        // Add a small delay so that the documents can be properly committed
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert_eq!(state.index.reader.searcher().num_docs(), 0);
    }
}
