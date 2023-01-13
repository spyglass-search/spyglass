use entities::get_library_stats;
use futures::StreamExt;
use jsonrpsee::core::Error;
use std::collections::HashSet;
use std::time::SystemTime;
use tracing::instrument;
use url::Url;

use entities::models::crawl_queue::CrawlStatus;
use entities::models::lens::LensType;
use entities::models::{
    bootstrap_queue, connection, crawl_queue, fetch_history, indexed_document, lens, tag,
};
use entities::schema::{DocFields, SearchDocument};
use entities::sea_orm::{prelude::*, sea_query, sea_query::Expr, QueryOrder, Set};
use shared::metrics::{self, Event};
use shared::request;
use shared::response::{
    AppStatus, InstallStatus, LensResult, ListConnectionResult, PluginResult, SearchLensesResp,
    SearchMeta, SearchResult, SearchResults, SupportedConnection, UserConnection,
};
use spyglass_plugin::SearchFilter;

use libgoog::{ClientType, Credentials, GoogClient};
use libspyglass::oauth::{self, connection_secret};
use libspyglass::plugin::PluginCommand;
use libspyglass::search::{lens::lens_to_filters, Searcher};
use libspyglass::state::AppState;
use libspyglass::task::{AppPause, CollectTask, ManagerCommand};

use super::auth::create_auth_listener;
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

    if let Some((client_id, client_secret, scopes)) = connection_secret(&api_id) {
        let mut listener = create_auth_listener().await;
        let client_type = match api_id.as_str() {
            "calendar.google.com" => ClientType::Calendar,
            "drive.google.com" => ClientType::Drive,
            _ => ClientType::Drive,
        };
        let mut client = GoogClient::new(
            client_type,
            &client_id,
            &client_secret,
            &format!("http://127.0.0.1:{}", listener.port()),
            Default::default(),
        )?;

        let request = client.authorize(&scopes);
        let _ = open::that(request.url.to_string());

        log::debug!("listening for auth code");
        if let Some(auth) = listener.listen(60 * 5).await {
            log::debug!("received oauth credentials: {:?}", auth);
            match client
                .token_exchange(&auth.code, &request.pkce_verifier)
                .await
            {
                Ok(token) => {
                    let mut creds = Credentials::default();
                    creds.refresh_token(&token);
                    let _ = client.set_credentials(&creds);

                    let user = client
                        .get_user()
                        .await
                        .expect("Unable to get account information");

                    let new_conn = connection::ActiveModel::new(
                        api_id.clone(),
                        user.email.clone(),
                        creds.access_token.secret().to_string(),
                        creds.refresh_token.map(|t| t.secret().to_string()),
                        creds
                            .expires_in
                            .map_or_else(|| None, |dur| Some(dur.as_secs() as i64)),
                        auth.scopes,
                    );
                    let res = new_conn.insert(&state.db).await;
                    match res {
                        Ok(_) => {
                            log::debug!("saved connection {} for {}", user.email.clone(), api_id);
                            let _ = state
                                .schedule_work(ManagerCommand::Collect(
                                    CollectTask::ConnectionSync {
                                        api_id,
                                        account: user.email,
                                    },
                                ))
                                .await;
                        }
                        Err(err) => log::error!("Unable to save connection: {}", err.to_string()),
                    }
                }
                Err(err) => log::error!("unable to exchange token: {}", err),
            }
        }

        Ok(())
    } else {
        Err(Error::Custom(format!(
            "Connection <{api_id}> not supported"
        )))
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
    match connection::Entity::find().all(&state.db).await {
        Ok(enabled) => {
            // TODO: Move this into a config / db table?
            let all_conns = oauth::supported_connections();
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
    let stats = get_library_stats(state.db).await.unwrap_or_default();
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
                        num_docs: lens_stats.indexed as u64,
                    }
                } else {
                    InstallStatus::Installing {
                        percent: lens_stats.percent_done(),
                        status: lens_stats.status_string(),
                    }
                }

            } else {
                InstallStatus::Finished { num_docs: 0 }
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
            }
        })
        .collect();

    lenses.sort_by(|x, y| x.label.to_lowercase().cmp(&y.label.to_lowercase()));

    Ok(lenses)
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

    let applied: Vec<SearchFilter> = futures::stream::iter(search_req.lenses.iter())
        .filter_map(|trigger| async {
            let vec = lens_to_filters(state.clone(), trigger).await;
            if vec.is_empty() {
                None
            } else {
                Some(vec)
            }
        })
        // Gather search filters
        .collect::<Vec<Vec<SearchFilter>>>()
        .await
        // Flatten
        .into_iter()
        .flatten()
        .collect::<Vec<SearchFilter>>();

    let docs =
        Searcher::search_with_lens(state.db.clone(), &applied, index, &search_req.query).await;

    let mut results: Vec<SearchResult> = Vec::new();
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

            if let Some(doc_id) = doc_id.as_text() {
                let indexed = indexed_document::Entity::find()
                    .filter(indexed_document::Column::DocId.eq(doc_id))
                    .one(&state.db)
                    .await;

                let crawl_uri = url.as_text().unwrap_or_default().to_string();

                if let Ok(Some(indexed)) = indexed {
                    let tags = indexed
                        .find_related(tag::Entity)
                        .all(&state.db)
                        .await
                        .unwrap_or_default()
                        .iter()
                        .map(|tag| (tag.label.as_ref().to_string(), tag.value.clone()))
                        .collect::<Vec<(String, String)>>();

                    let mut result = SearchResult {
                        doc_id: doc_id.to_string(),
                        domain: domain.as_text().unwrap_or_default().to_string(),
                        title: title.as_text().unwrap_or_default().to_string(),
                        crawl_uri: crawl_uri.clone(),
                        description: description.as_text().unwrap_or_default().to_string(),
                        url: indexed.open_url.unwrap_or(crawl_uri),
                        tags,
                        score,
                    };

                    result.description.truncate(256);
                    results.push(result);
                }
            }
        }
    }

    let wall_time_ms = SystemTime::now()
        .duration_since(start)
        .map_or_else(|_| 0, |duration| duration.as_millis() as u64);

    let meta = SearchMeta {
        query: search_req.query,
        num_docs: searcher.num_docs(),
        wall_time_ms,
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

    Ok(SearchResults { results, meta })
}

/// Search the user's installed lenses
#[instrument(skip(state))]
pub async fn search_lenses(
    state: AppState,
    param: request::SearchLensesParam,
) -> Result<SearchLensesResp, Error> {
    let mut results = Vec::new();

    let query_results = lens::Entity::find()
        // Filter either by the trigger name, which is configurable by the user.
        .filter(lens::Column::Trigger.like(&format!("%{}%", &param.query)))
        // Ignored disabled lenses
        .filter(lens::Column::IsEnabled.eq(true))
        // Order by trigger name, case insensitve
        .order_by_asc(Expr::cust("lower(trigger)"))
        .all(&state.db)
        .await;

    match query_results {
        Ok(query_results) => {
            for lens in query_results {
                let label = lens
                    .trigger
                    .map(|label| {
                        if label.is_empty() {
                            lens.name.clone()
                        } else {
                            label
                        }
                    })
                    .unwrap_or_else(|| lens.name.clone());

                results.push(LensResult {
                    author: lens.author,
                    name: lens.name,
                    label,
                    description: lens.description.unwrap_or_default(),
                    ..Default::default()
                });
            }

            Ok(SearchLensesResp { results })
        }
        Err(err) => {
            log::error!("Unable to search lenses: {:?}", err);
            Err(Error::Custom(err.to_string()))
        }
    }
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
pub async fn toggle_plugin(state: AppState, name: String) -> Result<(), Error> {
    // Find the plugin
    let plugin = lens::Entity::find()
        .filter(lens::Column::Name.eq(name))
        .filter(lens::Column::LensType.eq(LensType::Plugin))
        .one(&state.db)
        .await;

    if let Ok(Some(plugin)) = plugin {
        let mut updated: lens::ActiveModel = plugin.clone().into();
        let plugin_enabled = !plugin.is_enabled;
        updated.is_enabled = Set(plugin_enabled);
        let _ = updated.update(&state.db).await;

        let mut cmd_tx = state.plugin_cmd_tx.lock().await;
        match &mut *cmd_tx {
            Some(cmd_tx) => {
                let cmd = if plugin_enabled {
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
