use rocket::response::status::BadRequest;
use rocket::serde::json::Json;
use rocket::State;
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

#[post("/search", data = "<search_req>")]
pub async fn search(
    state: &State<AppState>,
    search_req: Json<request::SearchParam<'_>>,
) -> Result<Json<SearchResults>, BadRequest<String>> {
    let fields = Searcher::doc_fields();

    let index = state.index.lock().unwrap();
    let searcher = index.reader.searcher();

    let docs = Searcher::search_with_lens(
        &state.config.lenses,
        &index.index,
        &index.reader,
        &search_req.lenses,
        search_req.query,
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
        query: search_req.query.to_string(),
        num_docs: searcher.num_docs(),
        wall_time_ms: 1000,
    };

    Ok(Json(SearchResults { results, meta }))
}

/// Show the list of URLs in the queue and their status
#[get("/queue")]
pub async fn list_queue(
    state: &State<AppState>,
) -> Result<Json<response::ListQueue>, BadRequest<String>> {
    let db = &state.db;
    let queue = crawl_queue::Entity::find().all(db).await;

    match queue {
        Ok(queue) => Ok(Json(response::ListQueue { queue })),
        Err(err) => Err(BadRequest(Some(err.to_string()))),
    }
}

/// Add url to queue
#[post("/queue", data = "<queue_item>")]
pub async fn add_queue(
    state: &State<AppState>,
    queue_item: Json<request::QueueItemParam<'_>>,
) -> Result<&'static str, BadRequest<String>> {
    let db = &state.db;

    let parsed = Url::parse(queue_item.url).unwrap();
    let new_task = crawl_queue::ActiveModel {
        domain: Set(parsed.host_str().unwrap().to_string()),
        url: Set(queue_item.url.to_owned()),
        force_crawl: Set(queue_item.force_crawl),
        ..Default::default()
    };

    match new_task.insert(db).await {
        Ok(_) => Ok("ok"),
        Err(err) => Err(BadRequest(Some(err.to_string()))),
    }
}

pub async fn _get_current_status(state: &State<AppState>) -> AppStatus {
    let db = &state.db;
    let num_queued = crawl_queue::num_queued(db).await.unwrap();

    // Grab crawler status
    let app_state = &state.app_state;
    let paused_status = app_state.get("paused").unwrap();
    let is_paused = *paused_status == *"true";

    // Grab details about index
    let index = state.index.lock().unwrap();
    let reader = index.reader.searcher();

    AppStatus {
        num_docs: reader.num_docs(),
        num_queued,
        is_paused,
    }
}

/// Fun stats about index size, etc.
#[get("/status")]
pub async fn app_stats(state: &State<AppState>) -> Json<AppStatus> {
    Json(_get_current_status(state).await)
}

#[post("/status", data = "<update_status>")]
pub async fn update_app_status(
    state: &State<AppState>,
    update_status: Json<request::UpdateStatusParam>,
) -> Result<Json<AppStatus>, BadRequest<String>> {
    // Update status
    if update_status.toggle_pause.is_some() {
        let app_state = &state.app_state;
        let mut paused_status = app_state.get_mut("paused").unwrap();

        let current_status = paused_status.to_string() == "true";
        let updated_status = !current_status;
        *paused_status = updated_status.to_string();
    }

    Ok(Json(_get_current_status(state).await))
}

#[post("/lenses", data = "<param>")]
pub async fn search_lenses(
    state: &State<AppState>,
    param: Json<request::SearchLensesParam<'_>>,
) -> Result<Json<SearchLensesResp>, BadRequest<String>> {
    let mut results = Vec::new();

    for (lens_name, lens_info) in state.config.lenses.iter() {
        log::trace!("{} - {}", lens_name, param.query);
        if lens_name.starts_with(param.query) {
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

    Ok(Json(SearchLensesResp { results }))
}
