use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct InstallableLens {
    pub author: String,
    pub description: String,
    pub name: String,
    #[serde(default)]
    label: String,
    pub sha: String,
    pub download_url: String,
    pub html_url: String,
}

impl InstallableLens {
    pub fn identifier(&self) -> String {
        self.name.clone()
    }

    pub fn label(&self) -> String {
        if self.label.is_empty() {
            self.name.clone()
        } else {
            self.label.clone()
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum InstallStatus {
    NotInstalled,
    Finished { num_docs: u32 },
    Installing { percent: i32, status: String },
}

impl Default for InstallStatus {
    fn default() -> Self {
        Self::NotInstalled
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct LensResult {
    /// Author of this lens
    pub author: String,
    /// Unique identifier
    pub name: String,
    /// Human readable label
    pub label: String,
    /// Huamn readable description of the lens
    pub description: String,
    /// Used to determine whether a lens needs an update
    pub hash: String,
    /// For installed lenses.
    pub file_path: Option<PathBuf>,
    // Only relevant for installable lenses
    pub html_url: Option<String>,
    pub download_url: Option<String>,
    pub progress: InstallStatus,
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
    pub num_docs: u32,
    pub wall_time_ms: u32,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LibraryStats {
    pub lens_name: String,
    pub crawled: i32,
    pub enqueued: i32,
    pub indexed: i32,
}

impl LibraryStats {
    pub fn new(name: &str) -> Self {
        LibraryStats {
            lens_name: name.to_owned(),
            crawled: 0,
            enqueued: 0,
            indexed: 0,
        }
    }

    pub fn total_docs(&self) -> i32 {
        if self.enqueued == 0 {
            self.indexed
        } else {
            self.crawled + self.enqueued
        }
    }

    pub fn percent_done(&self) -> i32 {
        self.crawled * 100 / (self.crawled + self.enqueued)
    }

    pub fn status_string(&self) -> String {
        format!("Crawling {} of {}", self.enqueued, self.total_docs())
    }
}
