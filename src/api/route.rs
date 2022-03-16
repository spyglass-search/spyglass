use sea_orm::prelude::*;
use sea_orm::{DatabaseConnection, Set};

use rocket::response::status::BadRequest;
use rocket::serde::json::Json;
use rocket::State;
use serde::Deserialize;
use tantivy::{Index, IndexReader};
use url::Url;

use super::response;
use crate::api::response::SearchResult;
use crate::models::crawl_queue;
use crate::search::Searcher;
#[derive(Debug, Deserialize)]
pub struct SearchReq<'r> {
    pub term: &'r str,
}

#[post("/search", data = "<search_req>")]
pub async fn search(
    index: &State<Index>,
    reader: &State<IndexReader>,
    search_req: Json<SearchReq<'_>>,
) -> Result<Json<response::SearchResults>, BadRequest<String>> {
    let fields = Searcher::doc_fields();

    let searcher = reader.searcher();
    let docs = Searcher::search(index, reader, search_req.term);

    let mut results: Vec<SearchResult> = Vec::new();
    for (_score, doc_addr) in docs {
        let retrieved = searcher.doc(doc_addr).unwrap();

        let title = retrieved.get_first(fields.title).unwrap();
        let description = retrieved.get_first(fields.description).unwrap();
        let url = retrieved.get_first(fields.url).unwrap();

        let result = SearchResult {
            title: title.text().unwrap().to_string(),
            description: description.text().unwrap().to_string(),
            url: url.text().unwrap().to_string(),
        };

        results.push(result);
    }

    let meta = response::SearchMeta {
        query: search_req.term.to_string(),
        num_docs: searcher.num_docs(),
        wall_time_ms: 1000,
    };

    Ok(Json(response::SearchResults { results, meta }))
}

/// Show the list of URLs in the queue and their status
#[get("/queue")]
pub async fn list_queue(
    db: &State<DatabaseConnection>,
) -> Result<Json<response::ListQueue>, BadRequest<String>> {
    let db = db.inner();
    let queue = crawl_queue::Entity::find().all(db).await;

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
    db: &State<DatabaseConnection>,
    queue_item: Json<QueueItem<'_>>,
) -> Result<&'static str, BadRequest<String>> {
    let db = db.inner();

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

/// Fun stats about index size, etc.
#[get("/stats")]
pub fn app_stats(index_reader: &State<IndexReader>) -> Json<response::AppStats> {
    let index = index_reader.searcher();
    Json(response::AppStats {
        num_docs: index.num_docs(),
    })
}
