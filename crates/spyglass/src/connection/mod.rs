use anyhow::Result;
use entities::models::connection;
use entities::models::tag::TagPair;
use entities::sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use jsonrpsee::core::async_trait;
use libauth::{AccessToken, ApiClient, Credentials, RefreshToken};
use std::time::Duration;

use crate::crawler::{CrawlError, CrawlResult};
use crate::state::AppState;
use url::Url;

pub mod credentials;
pub mod gcal;
pub mod gdrive;
pub mod github;

#[async_trait]
pub trait Connection {
    fn id() -> String
    where
        Self: Sized + Send;

    /// Identifying user/account that this connection is for.
    fn user(&self) -> String;

    /// Default tags to be applied to all docs crawled w/ this connection
    fn default_tags(&self) -> Vec<TagPair>;

    /// Add URIs to crawl queue that are new/updated & remove ones that have
    /// been deleted.
    async fn sync(&mut self, state: &AppState);

    /// Get raw data for a URI
    async fn get(&mut self, uri: &Url) -> anyhow::Result<CrawlResult, CrawlError>;
}

async fn load_credentials(
    db: &DatabaseConnection,
    id: &str,
    account: &str,
) -> anyhow::Result<Credentials> {
    // Load credentials from db
    let creds = connection::get_by_id(db, &id, account)
        .await?
        .expect("No credentials matching that id");

    let credentials = Credentials {
        access_token: AccessToken::new(creds.access_token),
        refresh_token: creds.refresh_token.map(RefreshToken::new),
        requested_at: creds.granted_at,
        expires_in: creds.expires_in.map(|d| Duration::from_secs(d as u64)),
    };

    Ok(credentials)
}

// Update credentials in database whenever we refresh the token.
async fn handle_sync_credentials(
    api_client: &mut impl ApiClient,
    db: &DatabaseConnection,
    id: &str,
    account: &str,
) {
    let account = account.to_string();
    let db = db.clone();
    let id = id.to_string();

    api_client.set_on_refresh(move |new_creds| {
        log::debug!("received new credentials for {}:{}", id, account);

        let account = account.clone();
        let db = db.clone();
        let id = id.clone();
        let new_creds = new_creds.clone();
        tokio::spawn(async move {
            if let Ok(Some(conn)) = connection::get_by_id(&db, &id, &account).await {
                let mut update: connection::ActiveModel = conn.into();
                update.access_token = Set(new_creds.access_token.secret().to_string());
                // Refresh tokens are optionally sent
                if let Some(refresh_token) = new_creds.refresh_token {
                    update.refresh_token = Set(Some(refresh_token.secret().to_string()));
                }
                update.expires_in = Set(new_creds
                    .expires_in
                    .map_or_else(|| None, |dur| Some(dur.as_secs() as i64)));
                update.granted_at = Set(chrono::Utc::now());
                let res = update.save(&db).await;
                log::debug!("credentials updated: {:?}", res);
            }
        });
    });
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
        "api.github.com" => Ok(Box::new(
            github::GithubConnection::new(state, account)
                .await
                .expect("Unable to create github connection"),
        )),
        _ => Err(anyhow::anyhow!("Not suppported connection")),
    }
}
