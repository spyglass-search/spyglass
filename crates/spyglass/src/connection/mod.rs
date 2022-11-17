use anyhow::Result;
use jsonrpsee::core::async_trait;

use crate::crawler::{CrawlError, CrawlResult};
use crate::state::AppState;
use url::Url;

pub mod gcal;
pub mod gdrive;

#[async_trait]
pub trait Connection {
    fn id() -> String
    where
        Self: Sized + Send;

    /// Identifying user/account that this connection is for.
    fn user(&self) -> String;

    /// Add URIs to crawl queue that are new/updated & remove ones that have
    /// been deleted.
    async fn sync(&mut self, state: &AppState);

    /// Get raw data for a URI
    async fn get(&mut self, uri: &Url) -> anyhow::Result<CrawlResult, CrawlError>;
}

pub async fn load_connection(
    state: &AppState,
    api_id: &str,
    account: &str,
) -> Result<Box<dyn Connection + Send>> {
    match api_id {
        "calendar.google.com" => Ok(Box::new(
            gcal::GCalConnection::new(state, account)
                .await
                .expect("Unable to create gcal connection"),
        )),
        "drive.google.com" => Ok(Box::new(
            gdrive::DriveConnection::new(state, account)
                .await
                .expect("Unable to create gdrive connection"),
        )),
        _ => Err(anyhow::anyhow!("Not suppported connection")),
    }
}
