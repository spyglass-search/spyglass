use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use entities::models::connection;
use entities::models::tag::TagPair;
use entities::sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use jsonrpsee::core::async_trait;
use libauth::{AccessToken, ApiClient, Credentials, RefreshToken};
use libgithub::GithubClient;
use libgoog::{ClientType, GoogClient};
use libreddit::RedditClient;
use std::time::Duration;

use crate::crawler::{CrawlError, CrawlResult};
use crate::state::AppState;
use crate::task::{CollectTask, ManagerCommand};
use url::Url;

pub mod auth_server;
pub mod credentials;
pub mod gcal;
pub mod gdrive;
pub mod github;
pub mod reddit;

use auth_server::{create_auth_listener, AuthListener};
use credentials::connection_secret;

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
    async fn sync(&mut self, state: &AppState, last_synced_at: Option<DateTime<Utc>>);

    /// Get raw data for a URI
    async fn get(&mut self, uri: &Url) -> anyhow::Result<CrawlResult, CrawlError>;
}

/// Helper method used to access all configured api ids
pub async fn get_connection_ids(db: &DatabaseConnection) -> Vec<String> {
    let connections = connection::get_all_connections(db).await;
    connections
        .iter()
        .map(|connection| connection.api_id.clone())
        .collect::<Vec<String>>()
}

/// Helper method used to access the title and description for the specified api id
pub fn get_api_description(api_id: &str) -> Option<(&str, &str)> {
    match api_id {
        github::API_ID => Some((github::TITLE, github::DESCRIPTION)),
        gdrive::API_ID => Some((gdrive::TITLE, gdrive::DESCRIPTION)),
        gcal::API_ID => Some((gcal::TITLE, gcal::DESCRIPTION)),
        _ => None,
    }
}

/// Helper method used to convert from an api id to the associated lens id
pub fn api_id_to_lens(api_id: &str) -> Option<&str> {
    match api_id {
        github::API_ID => Some(github::LENS),
        gdrive::API_ID => Some(gdrive::LENS),
        gcal::API_ID => Some(gcal::LENS),
        _ => None,
    }
}

/// Load credentials from the db for an account, Errs if no credentials are found.
async fn load_credentials(
    db: &DatabaseConnection,
    id: &str,
    account: &str,
) -> anyhow::Result<Credentials> {
    let creds = connection::get_by_id(db, id, account)
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

/// Update credentials in database whenever we refresh the token.
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

/// Load a connection for sync/crawls
pub async fn load_connection(
    state: &AppState,
    api_id: &str,
    account: &str,
) -> Result<Box<dyn Connection + Send>> {
    match api_id {
        "calendar.google.com" => Ok(Box::new(gcal::GCalConnection::new(state, account).await?)),
        "drive.google.com" => Ok(Box::new(
            gdrive::DriveConnection::new(state, account).await?,
        )),
        "api.github.com" => Ok(Box::new(
            github::GithubConnection::new(state, account).await?,
        )),
        "oauth.reddit.com" => Ok(Box::new(
            reddit::RedditConnection::new(state, account).await?,
        )),
        _ => Err(anyhow::anyhow!("Not suppported connection")),
    }
}

async fn listen_for_token(
    state: &AppState,
    client: &mut impl ApiClient,
    listener: &mut AuthListener,
    scopes: &[String],
) -> Result<()> {
    let request = client.authorize(scopes);

    // Linux requires special checks if we're running inside an AppImage
    #[cfg(target_os = "linux")]
    let _ = crate::platform::linux_open(request.url.as_str());

    #[cfg(not(target_os = "linux"))]
    let _ = open::that(request.url.to_string());

    log::debug!("listening for auth code");
    let auth_code = listener
        .listen(60 * 5)
        .await
        .expect("No auth code detected");

    log::debug!("received oauth credentials: {:?}", auth_code);

    let token = client
        .token_exchange(&auth_code.code, &request.pkce_verifier)
        .await?;
    let mut creds = Credentials::default();
    creds.refresh_token(&token);
    let _ = client.set_credentials(&creds);

    let api_id = client.id();
    let account_id = client
        .account_id()
        .await
        .expect("Unable to get account information");
    let new_conn = connection::ActiveModel::new(
        api_id.clone(),
        account_id.clone(),
        creds.access_token.secret().to_string(),
        creds.refresh_token.map(|t| t.secret().to_string()),
        creds
            .expires_in
            .map_or_else(|| None, |dur| Some(dur.as_secs() as i64)),
        auth_code.scopes,
    );

    new_conn.insert(&state.db).await?;
    log::debug!("saved connection {} for {}", account_id, api_id);
    let _ = state
        .schedule_work(ManagerCommand::Collect(CollectTask::ConnectionSync {
            api_id,
            account: account_id,
            is_first_sync: true,
        }))
        .await;

    Ok(())
}

pub async fn handle_authorize_connection(state: &AppState, api_id: &str) -> Result<()> {
    // Grab the client id/secret for this connection
    let (client_id, client_secret, scopes) =
        connection_secret(api_id).expect("Unsupported connection");

    // FIX: Unfortunately reddit requires an explicit port.
    // Remove this once we have deep linking support.
    let port: Option<u16> = if api_id == "oauth.reddit.com" {
        Some(53124)
    } else {
        None
    };

    let mut listener = create_auth_listener(port).await;
    let redirect_uri = format!("http://127.0.0.1:{}", listener.port());

    let res = match api_id {
        "api.github.com" => {
            let mut client = GithubClient::new(
                &client_id,
                &client_secret,
                &redirect_uri,
                Default::default(),
            )?;
            listen_for_token(state, &mut client, &mut listener, &scopes).await
        }
        "calendar.google.com" => {
            let mut client = GoogClient::new(
                ClientType::Calendar,
                &client_id,
                &client_secret,
                &redirect_uri,
                Default::default(),
            )?;
            listen_for_token(state, &mut client, &mut listener, &scopes).await
        }
        "drive.google.com" => {
            let mut client = GoogClient::new(
                ClientType::Drive,
                &client_id,
                &client_secret,
                &redirect_uri,
                Default::default(),
            )?;
            listen_for_token(state, &mut client, &mut listener, &scopes).await
        }
        "oauth.reddit.com" => {
            let mut client = RedditClient::new(
                &client_id,
                &client_secret,
                &redirect_uri,
                Default::default(),
            )?;
            listen_for_token(state, &mut client, &mut listener, &scopes).await
        }
        _ => return Err(anyhow!("Unsupported connection")),
    };

    res?;

    Ok(())
}
