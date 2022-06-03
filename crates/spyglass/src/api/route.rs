use std::collections::HashMap;

use entities::sea_orm::prelude::*;
use entities::sea_orm::QueryOrder;
use entities::sea_orm::Set;
use jsonrpc_core::{Error, ErrorCode, Result};
use shared::response::LensResult;
use tracing::instrument;
use url::Url;

use shared::request;
use shared::response::{
    AppStatus, CrawlStats, QueueStatus, SearchLensesResp, SearchMeta, SearchResult, SearchResults,
};

use entities::models::{crawl_queue, indexed_document, lens};
use libspyglass::search::Searcher;
use libspyglass::state::AppState;

use super::response;

/// Add url to queue
#[instrument(skip(state))]
pub async fn add_queue(state: AppState, queue_item: request::QueueItemParam) -> Result<String> {
    let db = &state.db;

    let parsed = Url::parse(&queue_item.url).unwrap();
    let new_task = crawl_queue::ActiveModel {
        domain: Set(parsed.host_str().unwrap().to_string()),
        url: Set(queue_item.url.to_owned()),
        crawl_type: Set(crawl_queue::CrawlType::Normal),
        ..Default::default()
    };

    match new_task.insert(db).await {
        Ok(_) => Ok("ok".to_string()),
        Err(err) => Err(Error {
            code: ErrorCode::InternalError,
            message: err.to_string(),
            data: None,
        }),
    }
}

async fn _get_current_status(state: AppState) -> jsonrpc_core::Result<AppStatus> {
    // Grab crawler status
    let app_state = &state.app_state;
    let paused_status = app_state.get("paused").unwrap();
    let is_paused = *paused_status == *"true";

    // Grab details about index
    let index = state.index;
    let reader = index.reader.searcher();

    Ok(AppStatus {
        num_docs: reader.num_docs(),
        is_paused,
    })
}

/// Fun stats about index size, etc.
#[instrument(skip(state))]
pub async fn app_status(state: AppState) -> jsonrpc_core::Result<AppStatus> {
    _get_current_status(state).await
}

#[instrument(skip(state))]
pub async fn crawl_stats(state: AppState) -> jsonrpc_core::Result<CrawlStats> {
    let queue_stats = crawl_queue::queue_stats(&state.db).await;
    if let Err(err) = queue_stats {
        log::error!("{:?}", err);
        return Err(jsonrpc_core::Error::new(ErrorCode::InternalError));
    }

    let indexed_stats = indexed_document::indexed_stats(&state.db).await;
    if let Err(err) = indexed_stats {
        log::error!("{:?}", err);
        return Err(jsonrpc_core::Error::new(ErrorCode::InternalError));
    }

    let mut by_domain = HashMap::new();
    let queue_stats = queue_stats.unwrap();
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

    let indexed_stats = indexed_stats.unwrap();
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
pub async fn delete_doc(state: AppState, id: String) -> Result<()> {
    if let Ok(mut writer) = state.index.writer.lock() {
        if let Err(e) = Searcher::delete(&mut writer, &id) {
            log::error!("Unable to delete doc {} due to {}", id, e);
        } else {
            let _ = writer.commit();
        }
    }

    Ok(())
}

/// List of installed lenses
#[instrument(skip(state))]
pub async fn list_installed_lenses(state: AppState) -> Result<Vec<LensResult>> {
    let mut lenses: Vec<LensResult> = state
        .lenses
        .iter()
        .map(|lens| LensResult {
            author: lens.author.clone(),
            title: lens.name.clone(),
            description: lens.description.clone().unwrap_or_else(|| "".into()),
            ..Default::default()
        })
        .collect();

    lenses.sort_by(|x, y| x.title.cmp(&y.title));

    Ok(lenses)
}

/// Show the list of URLs in the queue and their status
#[instrument(skip(state))]
pub async fn list_queue(state: AppState) -> Result<response::ListQueue> {
    let db = &state.db;
    let queue = crawl_queue::Entity::find().all(db).await;

    match queue {
        Ok(queue) => Ok(response::ListQueue { queue }),
        Err(err) => Err(Error {
            code: ErrorCode::InternalError,
            message: err.to_string(),
            data: None,
        }),
    }
}

#[instrument(skip(state))]
pub async fn toggle_pause(state: AppState) -> jsonrpc_core::Result<AppStatus> {
    // Scope so that the app_state mutex is correctly released.
    {
        let app_state = &state.app_state;
        let mut paused_status = app_state.get_mut("paused").unwrap();

        let current_status = paused_status.to_string() == "true";
        let updated_status = !current_status;
        *paused_status = updated_status.to_string();
    }

    _get_current_status(state.clone()).await
}

/// Search the user's indexed documents
#[instrument(skip(state))]
pub async fn search(state: AppState, search_req: request::SearchParam) -> Result<SearchResults> {
    let fields = Searcher::doc_fields();

    let index = state.index;
    let searcher = index.reader.searcher();

    // Create a copy of the lenses for this search
    let mut lenses = HashMap::new();
    for entry in state.lenses.iter() {
        lenses.insert(entry.key().clone(), entry.value().clone());
    }

    let docs = Searcher::search_with_lens(
        &lenses,
        &index.index,
        &index.reader,
        &search_req.lenses,
        &search_req.query,
    );

    let mut results: Vec<SearchResult> = Vec::new();
    for (score, doc_addr) in docs {
        let retrieved = searcher.doc(doc_addr).unwrap();

        let doc_id = retrieved.get_first(fields.id).unwrap();
        let domain = retrieved.get_first(fields.domain).unwrap();
        let title = retrieved.get_first(fields.title).unwrap();
        let description = retrieved.get_first(fields.description).unwrap();
        let url = retrieved.get_first(fields.url).unwrap();

        let result = SearchResult {
            doc_id: doc_id.as_text().unwrap().to_string(),
            domain: domain.as_text().unwrap().to_string(),
            title: title.as_text().unwrap().to_string(),
            description: description.as_text().unwrap().to_string(),
            url: url.as_text().unwrap().to_string(),
            score,
        };

        results.push(result);
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
) -> Result<SearchLensesResp> {
    let mut results = Vec::new();

    let query_results = lens::Entity::find()
        .filter(lens::Column::Name.like(&format!("%{}%", &param.query)))
        .order_by_asc(lens::Column::Name)
        .all(&state.db)
        .await;

    if let Err(err) = query_results {
        log::error!("Unable to search lenses: {:?}", err);
        return Err(jsonrpc_core::Error::new(ErrorCode::InternalError));
    }

    let query_results = query_results.unwrap();
    for lens in query_results {
        results.push(LensResult {
            author: lens.author,
            title: lens.name,
            description: lens.description.unwrap_or_else(|| "".to_string()),
            ..Default::default()
        });
    }

    Ok(SearchLensesResp { results })
}
