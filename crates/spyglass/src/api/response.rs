use crate::models::crawl_queue;
use rocket::serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ListQueue {
    pub queue: Vec<crawl_queue::Model>,
}

#[derive(Serialize)]
pub struct AppStatus {
    pub num_docs: u64,
    pub is_paused: bool,
}

#[derive(Deserialize, Serialize)]
pub struct SearchMeta {
    pub query: String,
    pub num_docs: u64,
    pub wall_time_ms: u64,
}

#[derive(Deserialize, Serialize)]
pub struct SearchResult {
    pub title: String,
    pub description: String,
    pub url: String,
}

#[derive(Deserialize, Serialize)]
pub struct SearchResults {
    pub results: Vec<SearchResult>,
    pub meta: SearchMeta,
}
