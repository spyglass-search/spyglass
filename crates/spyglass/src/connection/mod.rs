use jsonrpsee::core::async_trait;

use crate::crawler::CrawlResult;
use crate::state::AppState;
use url::Url;

pub mod gcal;
pub mod gdrive;

#[async_trait]
pub trait Connection {
    fn id() -> String
    where
        Self: Sized;

    /// Identifying user/account that this connection is for.
    fn user(&self) -> String;

    /// Add URIs to crawl queue that are new/updated & remove ones that have
    /// been deleted.
    async fn sync(&mut self, state: &AppState);

    /// Get raw data for a URI
    async fn get(&mut self, uri: &Url) -> anyhow::Result<Option<CrawlResult>>;
}
