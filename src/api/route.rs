use rocket::response::status::BadRequest;
use rocket::serde::json::Json;
use rocket::State;
use serde::Deserialize;
use tantivy::IndexReader;

use super::response;
use crate::models::{CrawlQueue, DbPool};

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
    match CrawlQueue::insert(pool, queue_item.url, queue_item.force_crawl).await {
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
