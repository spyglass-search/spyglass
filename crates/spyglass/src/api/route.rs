use jsonrpc_core::{Error, ErrorCode, Result};
use sea_orm::prelude::*;
use sea_orm::Set;
use shared::response::LensResult;
use url::Url;

use shared::request;
use shared::response::{AppStatus, SearchLensesResp, SearchMeta, SearchResult, SearchResults};

use libspyglass::models::crawl_queue;
use libspyglass::search::Searcher;
use libspyglass::state::AppState;

use super::response;

#[allow(dead_code)]
pub async fn search(state: AppState, search_req: request::SearchParam) -> Result<SearchResults> {
    let fields = Searcher::doc_fields();

    let index = state.index.lock().unwrap();
    let searcher = index.reader.searcher();

    let docs = Searcher::search_with_lens(
        &state.config.lenses,
        &index.index,
        &index.reader,
        &search_req.lenses,
        &search_req.query,
    );

    let mut results: Vec<SearchResult> = Vec::new();
    for (score, doc_addr) in docs {
        let retrieved = searcher.doc(doc_addr).unwrap();

        let domain = retrieved.get_first(fields.domain).unwrap();
        let title = retrieved.get_first(fields.title).unwrap();
        let description = retrieved.get_first(fields.description).unwrap();
        let url = retrieved.get_first(fields.url).unwrap();

        let result = SearchResult {
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

/// Show the list of URLs in the queue and their status
#[allow(dead_code)]
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

/// Add url to queue
#[allow(dead_code)]
pub async fn add_queue(state: AppState, queue_item: request::QueueItemParam) -> Result<String> {
    let db = &state.db;

    let parsed = Url::parse(&queue_item.url).unwrap();
    let new_task = crawl_queue::ActiveModel {
        domain: Set(parsed.host_str().unwrap().to_string()),
        url: Set(queue_item.url.to_owned()),
        force_crawl: Set(queue_item.force_crawl),
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

pub async fn _get_current_status(state: AppState) -> jsonrpc_core::Result<AppStatus> {
    let db = &state.db;
    let num_queued = crawl_queue::num_queued(db).await.unwrap();

    // Grab crawler status
    let app_state = &state.app_state;
    let paused_status = app_state.get("paused").unwrap();
    let is_paused = *paused_status == *"true";

    // Grab details about index
    let index = state.index.lock().unwrap();
    let reader = index.reader.searcher();

    Ok(AppStatus {
        num_docs: reader.num_docs(),
        num_queued,
        is_paused,
    })
}

/// Fun stats about index size, etc.
pub async fn app_stats(state: AppState) -> jsonrpc_core::Result<AppStatus> {
    _get_current_status(state).await
}

pub async fn toggle_pause(state: AppState) -> jsonrpc_core::Result<AppStatus> {
    let app_state = &state.app_state;
    let mut paused_status = app_state.get_mut("paused").unwrap();

    let current_status = paused_status.to_string() == "true";
    let updated_status = !current_status;
    *paused_status = updated_status.to_string();

    _get_current_status(state.clone()).await
}

#[allow(dead_code)]
pub async fn search_lenses(
    state: AppState,
    param: request::SearchLensesParam,
) -> Result<SearchLensesResp> {
    let mut results = Vec::new();

    for (lens_name, lens_info) in state.config.lenses.iter() {
        log::trace!("{} - {}", lens_name, param.query);
        if lens_name.starts_with(&param.query) {
            results.push(LensResult {
                title: lens_name.to_owned(),
                description: lens_info
                    .description
                    .as_ref()
                    .unwrap_or(&"".to_string())
                    .to_owned(),
            })
        }
    }

    Ok(SearchLensesResp { results })
}
