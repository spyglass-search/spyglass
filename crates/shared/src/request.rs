use serde::{Deserialize, Serialize};

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
