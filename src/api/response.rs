use crate::models::CrawlQueue;
use rocket::serde::Serialize;

#[derive(Serialize)]
pub struct ListQueue {
    pub queue: Vec<CrawlQueue>,
}

#[derive(Serialize)]
pub struct AppStats {
    pub num_docs: u64,
}

#[derive(Serialize)]
pub struct SearchMeta {
    pub query: String,
    pub num_docs: u64,
    pub wall_time_ms: u64,
}

#[derive(Serialize)]
pub struct SearchResults {
    pub results: Vec<String>,
    pub meta: SearchMeta,
}
