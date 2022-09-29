use futures::StreamExt;
use jsonrpsee::core::Error;
use std::collections::HashMap;
use tracing::instrument;
use url::Url;

use entities::models::crawl_queue::CrawlStatus;
use entities::models::lens::LensType;
use entities::models::{bootstrap_queue, crawl_queue, fetch_history, indexed_document, lens};
use entities::schema::{DocFields, SearchDocument};
use entities::sea_orm::{prelude::*, sea_query, sea_query::Expr, QueryOrder, Set};
use shared::request;
use shared::response::{
    AppStatus, CrawlStats, LensResult, PluginResult, QueueStatus, SearchLensesResp, SearchMeta,
    SearchResult, SearchResults,
};
use spyglass_plugin::SearchFilter;

use libspyglass::plugin::PluginCommand;
use libspyglass::search::lens::lens_to_filters;
use libspyglass::search::Searcher;
use libspyglass::state::AppState;
use libspyglass::task::Command;

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

async fn _get_current_status(state: AppState) -> Result<AppStatus, Error> {
    // Grab details about index
    let index = state.index;
    let reader = index.reader.searcher();

    Ok(AppStatus {
        num_docs: reader.num_docs(),
    })
}

/// Fun stats about index size, etc.
#[instrument(skip(state))]
pub async fn app_status(state: AppState) -> Result<AppStatus, Error> {
    _get_current_status(state).await
}

#[instrument(skip(state))]
pub async fn crawl_stats(state: AppState) -> Result<CrawlStats, Error> {
    let queue_stats = crawl_queue::queue_stats(&state.db).await;
    if let Err(err) = queue_stats {
        log::error!("queue_stats {:?}", err);
        return Err(Error::Custom(err.to_string()));
    }

    let indexed_stats = indexed_document::indexed_stats(&state.db).await;
    if let Err(err) = indexed_stats {
        log::error!("index_stats {:?}", err);
        return Err(Error::Custom(err.to_string()));
    }

    let mut by_domain = HashMap::new();
    let queue_stats = queue_stats.expect("Invalid queue_stats");
    for stat in queue_stats {
        let entry = by_domain
            .entry(stat.domain)
            .or_insert_with(QueueStatus::default);
        match stat.status.as_str() {
            "Queued" => entry.num_queued += stat.count as u64,
            "Processing" => entry.num_processing += stat.count as u64,
            "Completed" => entry.num_completed += stat.count as u64,
            _ => {}
        }
    }

    let indexed_stats = indexed_stats.expect("Invalid indexed_stats");
    for stat in indexed_stats {
        let entry = by_domain
            .entry(stat.domain)
            .or_insert_with(QueueStatus::default);
        entry.num_indexed += stat.count as u64;
    }

    let by_domain = by_domain
        .into_iter()
        .filter(|(_, stats)| stats.total() >= 10)
        .collect();

    Ok(CrawlStats { by_domain })
}

/// Remove a doc from the index
#[instrument(skip(state))]
pub async fn delete_doc(state: AppState, id: String) -> Result<(), Error> {
    if let Ok(mut writer) = state.index.writer.lock() {
        if let Err(e) = Searcher::delete(&mut writer, &id) {
            log::error!("Unable to delete doc {} due to {}", id, e);
        } else {
            let _ = writer.commit();
        }
    }

    // Remove from indexed_doc table
    if let Ok(Some(model)) = indexed_document::Entity::find()
        .filter(indexed_document::Column::DocId.eq(id))
        .one(&state.db)
        .await
    {
        let _ = model.delete(&state.db).await;
    }

    Ok(())
}

/// Remove a domain from crawl queue & index
#[instrument(skip(state))]
pub async fn delete_domain(state: AppState, domain: String) -> Result<(), Error> {
    // Remove domain from bootstrap queue
    if let Err(err) =
        bootstrap_queue::dequeue(&state.db, format!("https://{}", domain).as_str()).await
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
        .filter(indexed_document::Column::Domain.eq(domain))
        .all(&state.db)
        .await;

    if let Ok(indexed) = indexed {
        for result in indexed {
            if let Ok(mut writer) = state.index.writer.lock() {
                let _ = Searcher::delete(&mut writer, &result.doc_id);
                let _ = writer.commit();
            }
            let _ = result.delete(&state.db).await;
        }
    }

    Ok(())
}

/// List of installed lenses
#[instrument(skip(state))]
pub async fn list_installed_lenses(state: AppState) -> Result<Vec<LensResult>, Error> {
    let mut lenses: Vec<LensResult> = state
        .lenses
        .iter()
        .map(|lens| LensResult {
            author: lens.author.clone(),
            title: lens.name.clone(),
            description: lens.description.clone().unwrap_or_else(|| "".into()),
            hash: lens.hash.clone(),
            file_path: Some(lens.file_path.clone()),
            ..Default::default()
        })
        .collect();

    lenses.sort_by(|x, y| x.title.cmp(&y.title));

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
            sea_query::Expr::value(sea_query::Value::String(Some(Box::new(
                CrawlStatus::Queued.to_string(),
            )))),
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
        Searcher::search_with_lens(state.db.clone(), &applied, &index.reader, &search_req.query)
            .await;

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

            let result = SearchResult {
                doc_id: doc_id.as_text().unwrap_or_default().to_string(),
                domain: domain.as_text().unwrap_or_default().to_string(),
                title: title.as_text().unwrap_or_default().to_string(),
                description: description.as_text().unwrap_or_default().to_string(),
                url: url.as_text().unwrap_or_default().to_string(),
                score,
            };

            results.push(result);
        }
    }

    let meta = SearchMeta {
        query: search_req.query,
        num_docs: searcher.num_docs(),
        wall_time_ms: 1000,
    };

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
                    .unwrap_or(lens.name);

                results.push(LensResult {
                    author: lens.author,
                    title: label,
                    description: lens.description.unwrap_or_else(|| "".to_string()),
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
    if let Some(sender) = state.crawler_cmd_tx.lock().await.as_ref() {
        let _ = sender.send(if is_paused {
            Command::PauseCrawler
        } else {
            Command::RunCrawler
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
