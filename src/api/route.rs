use rocket::response::status::BadRequest;
use rocket::serde::json::Json;
use rocket::State;
use serde::Deserialize;
use tantivy::IndexReader;

use super::response;
use crate::crawler::Crawler;
use crate::models::{CrawlQueue, DbPool};

#[derive(Debug, Deserialize)]
pub struct SearchReq<'r> {
    pub term: &'r str
}

#[get("/search", data = "<search_req>")]
pub async fn search(
    _searcher: &State<IndexReader>,
    search_req: Json<SearchReq<'_>>,
) -> Result<Json<response::SearchResults>, BadRequest<String>> {
    let results = Vec::new();
    let meta = response::SearchMeta {
        query: search_req.term.to_string(),
        num_docs: 0,
        wall_time_ms: 1000,
    };

    Ok(Json(response::SearchResults { results, meta }))
}

/// Show the list of URLs in the queue and their status
#[get("/queue")]
pub async fn list_queue(
    pool: &State<DbPool>,
) -> Result<Json<response::ListQueue>, BadRequest<String>> {
    let queue = CrawlQueue::list(pool, None).await;

    match queue {
        Ok(queue) => Ok(Json(response::ListQueue { queue })),
        Err(err) => Err(BadRequest(Some(err.to_string()))),
    }
}

#[derive(Debug, Deserialize)]
pub struct QueueItem<'r> {
    pub url: &'r str,
    pub force_crawl: bool,
}

/// Add url to queue
#[post("/queue", data = "<queue_item>")]
pub async fn add_queue(
    pool: &State<DbPool>,
    queue_item: Json<QueueItem<'_>>,
) -> Result<&'static str, BadRequest<String>> {
    //
    // Add to queue instead of directly calling the fetch function
    // CrawlQueue::insert(pool, queue_item.url, queue_item.force_crawl).await {
    //
    match Crawler::fetch(pool, queue_item.url, queue_item.force_crawl).await {
        Ok(()) => Ok("ok"),
        Err(err) => Err(BadRequest(Some(err.to_string()))),
    }
}

/// Fun stats about index size, etc.
#[get("/stats")]
pub fn app_stats(index_reader: &State<IndexReader>) -> Json<response::AppStats> {
    let index = index_reader.searcher();
    Json(response::AppStats {
        num_docs: index.num_docs(),
    })
}
