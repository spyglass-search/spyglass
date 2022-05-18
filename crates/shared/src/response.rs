use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct QueueStatus {
    pub num_queued: u64,
    pub num_processing: u64,
    pub num_completed: u64,
    pub num_indexed: u64,
}

impl QueueStatus {
    pub fn total(&self) -> u64 {
        self.num_completed + self.num_indexed + self.num_processing + self.num_queued
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AppStatus {
    pub num_docs: u64,
    pub is_paused: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CrawlStats {
    pub by_domain: Vec<(String, QueueStatus)>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchMeta {
    pub query: String,
    pub num_docs: u64,
    pub wall_time_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchResult {
    pub domain: String,
    pub title: String,
    pub description: String,
    pub url: String,
    pub score: f32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchResults {
    pub results: Vec<SearchResult>,
    pub meta: SearchMeta,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LensResult {
    pub title: String,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchLensesResp {
    pub results: Vec<LensResult>,
}
