use crate::url_to_file_path;
use num_format::{Buffer, Locale};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use url::Url;

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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct UserConnection {
    pub id: String,
    pub account: String,
    pub is_syncing: bool,
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
    #[serde(default)]
    pub categories: Vec<String>,
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

impl InstallStatus {
    pub fn is_installing(&self) -> bool {
        matches!(
            self,
            Self::Installing {
                percent: _,
                status: _
            }
        )
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum LensType {
    #[default]
    Lens,
    Plugin,
    API,
    Internal,
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
    #[serde(default)]
    pub categories: Vec<String>,
    pub html_url: Option<String>,
    pub download_url: Option<String>,
    pub progress: InstallStatus,
    pub lens_type: LensType,
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

// The search result template is used to provide extra
// fields for action template expansion. This provides
// additional power for template expansion without the need
// for complicated template logic
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SearchResultTemplate {
    /// Document ID
    pub doc_id: String,
    /// URI used to crawl this result
    pub crawl_uri: String,
    pub domain: String,
    pub title: String,
    pub description: String,
    pub url: String,
    pub open_url: String,
    pub tags: Vec<(String, String)>,
    pub score: f32,
    pub url_schema: String,
    pub url_userinfo: String,
    pub url_parent: String,
    pub url_port: u16,
    pub url_path: Vec<String>,
    pub url_path_length: u32,
    pub url_query: String,
}

impl From<SearchResult> for SearchResultTemplate {
    fn from(value: SearchResult) -> Self {
        let mut result = Self {
            doc_id: value.doc_id,
            crawl_uri: value.crawl_uri,
            domain: value.domain,
            title: value.title,
            description: value.description,
            url: value.url.clone(),
            open_url: String::from(""),
            tags: value.tags,
            score: value.score,
            url_schema: String::from(""),
            url_userinfo: String::from(""),
            url_port: 0,
            url_path: Vec::new(),
            url_parent: String::from(""),
            url_path_length: 0,
            url_query: String::from(""),
        };

        if let Some((parent, _)) = value.url.rsplit_once('/') {
            result.url_parent = parent.to_string();
        }

        if let Ok(mut url) = Url::parse(&value.url) {
            result.url_schema = url.scheme().to_owned();
            result.url_userinfo = url.username().to_owned();
            result.url_port = url.port().unwrap_or(0);

            if let Some(segments) = url.path_segments().map(|c| c.collect::<Vec<_>>()) {
                result.url_path = segments
                    .iter()
                    .filter_map(|s| {
                        if !s.is_empty() {
                            Some(s.to_string())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<String>>();
                result.url_path_length = segments.len() as u32;
            }

            result.url_query = String::from(url.query().unwrap_or(""));

            if url.scheme() == "file" {
                let _ = url.set_host(None);
                result.open_url = url_to_file_path(url.path(), true);
            } else {
                result.open_url = result.url.clone();
            }
        }
        result
    }
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LibraryStats {
    pub lens_name: String,
    pub crawled: i32,
    pub enqueued: i32,
    pub indexed: i32,
    pub failed: i32,
}

impl LibraryStats {
    pub fn new(name: &str) -> Self {
        LibraryStats {
            lens_name: name.to_owned(),
            crawled: 0,
            enqueued: 0,
            indexed: 0,
            failed: 0,
        }
    }

    pub fn total_docs(&self) -> i32 {
        if self.enqueued == 0 {
            self.indexed
        } else {
            self.crawled + self.enqueued + self.failed
        }
    }

    pub fn percent_done(&self) -> i32 {
        self.crawled * 100 / (self.crawled + self.enqueued)
    }

    pub fn status_string(&self) -> String {
        // For plugins/connections where we don't know exactly how many there are
        if self.enqueued == 0 {
            let mut indexed = Buffer::default();
            indexed.write_formatted(&self.indexed, &Locale::en);
            format!("Added {indexed} of many")
        } else {
            let mut indexed = Buffer::default();
            let mut total = Buffer::default();

            indexed.write_formatted(&self.indexed, &Locale::en);
            total.write_formatted(&self.total_docs(), &Locale::en);
            format!("Added {indexed} of {total}")
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DefaultIndices {
    pub file_paths: Vec<PathBuf>,
    pub extensions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SimilarityResultPayload {
    pub title: String,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SimilaritySearchResult {
    pub id: usize,
    pub version: usize,
    pub score: f32,
    pub payload: SimilarityResultPayload,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DocMetadata {
    pub doc_id: String,
    pub title: String,
    pub open_url: String,
}

/// From backend -> client for display
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SendToAskClippyPayload {
    pub question: Option<String>,
    pub docs: Vec<DocMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ChatUpdate {
    LoadingModel,
    LoadingPrompt,
    SearchingDocuments,
    DocumentContextAdded(Vec<DocMetadata>),
    GeneratingContext,
    EndOfText,
    Error(String),
    Token(String),
}
