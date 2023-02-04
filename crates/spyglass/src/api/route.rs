use directories::UserDirs;
use entities::get_library_stats;
use entities::models::crawl_queue::CrawlStatus;
use entities::models::lens::LensType;
use entities::models::tag::TagType;
use entities::models::{
    bootstrap_queue, connection::get_all_connections, crawl_queue, fetch_history, indexed_document,
    lens, tag,
};
use entities::schema::{DocFields, SearchDocument};
use entities::sea_orm;
use entities::sea_orm::{prelude::*, sea_query, sea_query::Expr, QueryOrder, Set};
use entities::sea_orm::{FromQueryResult, JoinType, QuerySelect};
use jsonrpsee::core::Error;
use libspyglass::connection::{self, credentials, handle_authorize_connection};
use libspyglass::filesystem;
use libspyglass::plugin::PluginCommand;
use libspyglass::search::Searcher;
use libspyglass::state::AppState;
use libspyglass::task::{AppPause, CleanupTask, ManagerCommand};
use shared::config::Config;
use shared::metrics::{self, Event};
use shared::request;
use shared::response::{
    AppStatus, DefaultIndices, InstallStatus, LensResult, LibraryStats, ListConnectionResult,
    PluginResult, SearchLensesResp, SearchMeta, SearchResult, SearchResults, SupportedConnection,
    UserConnection,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;
use tracing::instrument;
use url::Url;

use super::response;

/// Add url to queue
#[allow(dead_code)]
#[instrument(skip(state))]
pub async fn add_queue(
    state: AppState,
    queue_item: request::QueueItemParam,
) -> Result<String, Error> {
    let db = &state.db;

    if let Ok(parsed) = Url::parse(&queue_item.url) {
        let new_task = crawl_queue::ActiveModel {
            domain: Set(parsed.host_str().expect("Invalid host str").to_string()),
            url: Set(queue_item.url.to_owned()),
            crawl_type: Set(crawl_queue::CrawlType::Normal),
            ..Default::default()
        };

        return match new_task.insert(db).await {
            Ok(_) => Ok("ok".to_string()),
            Err(err) => Err(Error::Custom(err.to_string())),
        };
    }

    Ok("ok".to_string())
}

#[instrument(skip(state))]
pub async fn authorize_connection(state: AppState, api_id: String) -> Result<(), Error> {
    log::debug!("authorizing <{}>", api_id);
    state
        .metrics
        .track(Event::AuthorizeConnection {
            api_id: api_id.clone(),
        })
        .await;

    if let Err(err) = handle_authorize_connection(&state, &api_id).await {
        Err(Error::Custom(format!(
            "Unable to authorize {api_id}: {err}"
        )))
    } else {
        Ok(())
    }
}

/// Fun stats about index size, etc.
#[instrument(skip(state))]
pub async fn app_status(state: AppState) -> Result<AppStatus, Error> {
    // Grab details about index
    let index = state.index;
    let reader = index.reader.searcher();

    Ok(AppStatus {
        num_docs: reader.num_docs(),
    })
}

/// Remove a doc from the index
#[instrument(skip(state))]
pub async fn delete_doc(state: AppState, id: String) -> Result<(), Error> {
    if let Err(e) = Searcher::delete_by_id(&state, &id).await {
        log::error!("Unable to delete doc {} due to {}", id, e);
        return Err(Error::Custom(e.to_string()));
    }
    let _ = Searcher::save(&state).await;
    Ok(())
}

/// Remove a domain from crawl queue & index
#[instrument(skip(state))]
pub async fn delete_domain(state: AppState, domain: String) -> Result<(), Error> {
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
        for result in indexed {
            let _ = Searcher::delete_by_id(&state, &result.doc_id).await;
        }
        let _ = Searcher::save(&state).await;

        log::debug!("removed {} items from index", indexed_count);
    }

    Ok(())
}

#[instrument(skip(state))]
pub async fn list_connections(state: AppState) -> Result<ListConnectionResult, Error> {
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
        Err(err) => Err(Error::Custom(err.to_string())),
    }
}

/// List of installed lenses
#[instrument(skip(state))]
pub async fn list_installed_lenses(state: AppState) -> Result<Vec<LensResult>, Error> {
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
                html_url: None,
                download_url: None,
                lens_type: shared::response::LensType::Lens,
            }
        })
        .collect();

    if let Some(result) =
        build_filesystem_information(&state, stats.get(filesystem::FILES_LENS)).await
    {
        lenses.push(result);
    }

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
                        status: "Syncing...".into(),
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
    lens_stats: Option<&LibraryStats>,
) -> Option<LensResult> {
    if filesystem::is_watcher_enabled(&state.db).await {
        let watcher = state.file_watcher.lock().await;
        let ref_watcher = watcher.as_ref();
        if let Some(watcher) = ref_watcher {
            let total_paths = watcher.processed_path_count().await as u32;
            let mut indexed: u32 = 0;
            let mut failed: u32 = 0;
            if let Some(stats) = lens_stats {
                indexed = stats.indexed as u32;
                failed = stats.failed as u32;
            }
            let total_finished = indexed + failed;

            let path = watcher.initializing_path().await;
            let mut status = InstallStatus::Finished { num_docs: indexed };
            if total_finished < total_paths {
                let percent = ((indexed * 100) / total_paths) as i32;
                let status_msg = match path {
                    Some(path) => format!("Walking path {path}"),
                    None => String::from("Processing local files..."),
                };
                status = InstallStatus::Installing {
                    percent,
                    status: status_msg,
                }
            }

            Some(LensResult {
            author: String::from("spyglass-search"),
            name: String::from("local-file-system"),
            label: String::from("Local File System"),
            description: String::from("Provides indexing for local files. All content is processed locally and stored locally! Contents of supported file types will be indexed. All unsupported file types and directories will be indexed based on their path, name and extension."),
            hash: String::from("N/A"),
            file_path: None,
            progress: status,
            html_url: None,
            download_url: None,
            lens_type: shared::response::LensType::Internal
        })
        } else {
            None
        }
    } else {
        None
    }
}

pub async fn list_plugins(state: AppState) -> Result<Vec<PluginResult>, Error> {
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
pub async fn list_queue(state: AppState) -> Result<response::ListQueue, Error> {
    let db = &state.db;
    let queue = crawl_queue::Entity::find().all(db).await;

    match queue {
        Ok(queue) => Ok(response::ListQueue { queue }),
        Err(err) => Err(Error::Custom(err.to_string())),
    }
}

#[instrument(skip(state))]
pub async fn recrawl_domain(state: AppState, domain: String) -> Result<(), Error> {
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

/// Search the user's indexed documents
#[instrument(skip(state))]
pub async fn search(
    state: AppState,
    search_req: request::SearchParam,
) -> Result<SearchResults, Error> {
    state
        .metrics
        .track(metrics::Event::Search {
            filters: search_req.lenses.clone(),
        })
        .await;

    let start = SystemTime::now();
    let fields = DocFields::as_fields();

    let index = &state.index;
    let searcher = index.reader.searcher();

    let tags = tag::Entity::find()
        .filter(tag::Column::Label.eq(tag::TagType::Lens))
        .filter(tag::Column::Value.is_in(search_req.lenses))
        .all(&state.db)
        .await
        .unwrap_or_default();
    let tag_ids = tags
        .iter()
        .map(|model| model.id as u64)
        .collect::<Vec<u64>>();

    let docs =
        Searcher::search_with_lens(state.db.clone(), &tag_ids, index, &search_req.query).await;

    let mut results: Vec<SearchResult> = Vec::new();
    let mut missing: Vec<(String, String)> = Vec::new();
    for (score, doc_addr) in docs {
        if let Ok(retrieved) = searcher.doc(doc_addr) {
            let doc_id = retrieved
                .get_first(fields.id)
                .expect("Missing doc_id in schema");
            let domain = retrieved
                .get_first(fields.domain)
                .expect("Missing domain in schema");
            let title = retrieved
                .get_first(fields.title)
                .expect("Missing title in schema");
            let description = retrieved
                .get_first(fields.description)
                .expect("Missing description in schema");
            let url = retrieved
                .get_first(fields.url)
                .expect("Missing url in schema");

            log::debug!("Got id with url {:?} {:?}", doc_id, url);
            if let Some(doc_id) = doc_id.as_text() {
                let indexed = indexed_document::Entity::find()
                    .filter(indexed_document::Column::DocId.eq(doc_id))
                    .one(&state.db)
                    .await;

                let crawl_uri = url.as_text().unwrap_or_default().to_string();
                match indexed {
                    Ok(Some(indexed)) => {
                        let tags = indexed
                            .find_related(tag::Entity)
                            .all(&state.db)
                            .await
                            .unwrap_or_default()
                            .iter()
                            .map(|tag| (tag.label.as_ref().to_string(), tag.value.clone()))
                            .collect::<Vec<(String, String)>>();

                        let result = SearchResult {
                            doc_id: doc_id.to_string(),
                            domain: domain.as_text().unwrap_or_default().to_string(),
                            title: title.as_text().unwrap_or_default().to_string(),
                            crawl_uri: crawl_uri.clone(),
                            description: description.as_text().unwrap_or_default().to_string(),
                            url: indexed.open_url.unwrap_or(crawl_uri),
                            tags,
                            score,
                        };

                        results.push(result);
                    }
                    _ => {
                        missing.push((doc_id.to_owned(), crawl_uri.to_owned()));
                    }
                }
            }
        }
    }

    let wall_time_ms = SystemTime::now()
        .duration_since(start)
        .map_or_else(|_| 0, |duration| duration.as_millis() as u64);

    let meta = SearchMeta {
        query: search_req.query,
        num_docs: searcher.num_docs() as u32,
        wall_time_ms: wall_time_ms as u32,
    };

    let domains: HashSet<String> = HashSet::from_iter(results.iter().map(|r| r.domain.clone()));
    state
        .metrics
        .track(metrics::Event::SearchResult {
            num_results: results.len(),
            domains: domains.iter().cloned().collect(),
            wall_time_ms,
        })
        .await;

    // Send cleanup task for any missing docs
    if !missing.is_empty() {
        let mut cmd_tx = state.manager_cmd_tx.lock().await;
        match &mut *cmd_tx {
            Some(cmd_tx) => {
                let _ = cmd_tx.send(ManagerCommand::CleanupDatabase(CleanupTask {
                    missing_docs: missing,
                }));
            }
            None => {}
        }
    }

    Ok(SearchResults { results, meta })
}

#[derive(FromQueryResult)]
struct LensSearch {
    author: Option<String>,
    name: String,
    description: Option<String>,
}

/// Search the user's installed lenses
#[instrument(skip(state))]
pub async fn search_lenses(
    state: AppState,
    param: request::SearchLensesParam,
) -> Result<SearchLensesResp, Error> {
    let mut results = Vec::new();
    let query_result = tag::Entity::find()
        .column_as(tag::Column::Value, "name")
        .column_as(lens::Column::Author, "author")
        .column_as(lens::Column::Description, "description")
        .filter(tag::Column::Label.eq(TagType::Lens))
        .filter(tag::Column::Value.like(&format!("%{}%", &param.query)))
        // Pull in lens metadata
        .join_rev(
            JoinType::LeftJoin,
            lens::Entity::belongs_to(tag::Entity)
                .from(lens::Column::Name)
                .to(tag::Column::Value)
                .into(),
        )
        // Order by trigger name, case insensitve
        .order_by_asc(Expr::cust("lower(value)"))
        .into_model::<LensSearch>()
        .all(&state.db)
        .await
        .unwrap_or_default();

    for lens in query_result {
        let label = lens.name.clone();
        results.push(LensResult {
            author: lens.author.unwrap_or("spyglass-search".into()),
            name: label.clone(),
            label,
            description: lens.description.unwrap_or_default(),
            ..Default::default()
        });
    }

    Ok(SearchLensesResp { results })
}

#[instrument(skip(state))]
pub async fn toggle_pause(state: AppState, is_paused: bool) -> Result<(), Error> {
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

#[instrument(skip(state))]
pub async fn toggle_plugin(state: AppState, name: String, enabled: bool) -> Result<(), Error> {
    // Find the plugin
    let plugin = lens::Entity::find()
        .filter(lens::Column::Name.eq(name))
        .filter(lens::Column::LensType.eq(LensType::Plugin))
        .one(&state.db)
        .await;

    if let Ok(Some(plugin)) = plugin {
        let mut updated: lens::ActiveModel = plugin.clone().into();
        updated.is_enabled = Set(enabled);
        let _ = updated.update(&state.db).await;

        let mut cmd_tx = state.plugin_cmd_tx.lock().await;
        match &mut *cmd_tx {
            Some(cmd_tx) => {
                let cmd = if enabled {
                    PluginCommand::EnablePlugin(plugin.name)
                } else {
                    PluginCommand::DisablePlugin(plugin.name)
                };

                let _ = cmd_tx.send(cmd).await;
            }
            None => {}
        }
    }

    Ok(())
}

#[instrument(skip(state))]
pub async fn uninstall_lens(state: AppState, config: &Config, name: &str) -> Result<(), Error> {
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
        if let Err(err) = Searcher::delete_many_by_id(&state, &doc_ids, true).await {
            return Err(Error::Custom(err.to_string()));
        } else {
            let _ = Searcher::save(&state).await;
        }
    }

    // -- remove from crawl queue & bootstrap table
    if let Err(err) = crawl_queue::delete_by_lens(state.db.clone(), name).await {
        return Err(Error::Custom(err.to_string()));
    }

    // - remove seed urls from bootstrap queue table
    if let Some((_, config)) = config {
        for url in &config.urls {
            let _ = bootstrap_queue::dequeue(&state.db, url).await;
        }

        for url in &config.domains {
            let _ = bootstrap_queue::dequeue(&state.db, url).await;
        }
    }
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
    DefaultIndices { file_paths }
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
    use libspyglass::search::{DocumentUpdate, Searcher};
    use libspyglass::state::AppState;
    use shared::config::{Config, LensConfig};

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

        if let Ok(mut writer) = state.index.writer.lock() {
            Searcher::upsert_document(
                &mut writer,
                DocumentUpdate {
                    doc_id: Some("test_id".into()),
                    title: "test title",
                    description: "test desc",
                    domain: "example.com",
                    url: "https://example.com/test",
                    content: "test content",
                    tags: &None,
                },
            )
            .expect("Unable to add doc");
        }
        let _ = Searcher::save(&state).await;

        let doc = indexed_document::ActiveModel {
            domain: Set("example.com".into()),
            url: Set("https://example.com/test".into()),
            doc_id: Set("test_id".into()),
            ..Default::default()
        };

        let model = doc.insert(&db).await.expect("Unable to insert doc");
        let doc: indexed_document::ActiveModel = model.into();
        doc.insert_tags(&db, &[(TagType::Lens, lens.name.clone())])
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
        assert_eq!(state.index.reader.searcher().num_docs(), 0);
    }
}
