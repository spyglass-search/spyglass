use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SupportedConnection {
    pub id: String,
    pub label: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserConnection {
    pub id: String,
    pub account: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ListConnectionResult {
    pub supported: Vec<SupportedConnection>,
    pub user_connections: Vec<UserConnection>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CrawlStats {
    pub by_domain: Vec<(String, QueueStatus)>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct InstallableLens {
    pub author: String,
    pub description: String,
    pub name: String,
    pub sha: String,
    pub download_url: String,
    pub html_url: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct LensResult {
    pub author: String,
    pub title: String,
    pub description: String,
    // Used to determine whether a lens needs an update
    pub hash: String,
    // For installed lenses.
    pub file_path: Option<PathBuf>,
    // Only relevant for installable lenses
    pub html_url: Option<String>,
    pub download_url: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PluginResult {
    pub author: String,
    pub title: String,
    pub description: String,
    pub is_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchMeta {
    pub query: String,
    pub num_docs: u64,
    pub wall_time_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SearchResult {
    /// Document ID
    pub doc_id: String,
    /// URI used to crawl this result
    pub crawl_uri: String,
    pub domain: String,
    pub title: String,
    pub description: String,
    pub url: String,
    pub tags: Vec<(String, String)>,
    pub score: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchResults {
    pub results: Vec<SearchResult>,
    pub meta: SearchMeta,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SearchLensesResp {
    pub results: Vec<LensResult>,
}
