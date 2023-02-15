use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchParam {
    pub lenses: Vec<String>,
    pub query: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchLensesParam {
    pub query: String,
}

#[derive(Debug, Deserialize)]
pub struct QueueItemParam {
    pub url: String,
    pub force_crawl: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatusParam {
    pub toggle_pause: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum RawDocType {
    /// Raw HTML, typically from a page the user is currently on.
    Html,
    /// Raw text
    Text,
    /// No content, just a URL, to be processed by the crawler.
    Url,
}

#[derive(Debug, Deserialize, Display, EnumString, Serialize, PartialEq, Eq)]
pub enum RawDocSource {
    #[strum(serialize = "cli")]
    Cli,
    #[strum(serialize = "web_extension")]
    WebExtension,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RawDocumentRequest {
    pub url: String,
    pub content: String,
    pub doc_type: RawDocType,
    pub source: RawDocSource,
    pub tags: Vec<(String, String)>,
}
