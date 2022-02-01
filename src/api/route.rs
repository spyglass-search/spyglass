use rocket::serde::json::Json;
use rocket::State;
use rocket::response::status::BadRequest;
use tantivy::IndexReader;

use crate::models::{CrawlQueue, DbPool};
use super::response;

/// Show the list of URLs in the queue and their status
#[get("/queue")]
pub async fn list_queue(pool: &State<DbPool>) -> Result<Json<response::ListQueue>, BadRequest<String>> {
    let queue = CrawlQueue::list(pool)
        .await;

    match queue {
        Ok(queue) => Ok(Json(response::ListQueue { queue })),
        Err(err) => Err(BadRequest(Some(err.to_string())))
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
