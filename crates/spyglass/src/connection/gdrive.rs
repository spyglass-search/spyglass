use entities::models::crawl_queue::{CrawlType, EnqueueSettings};
use entities::sea_orm::{ActiveModelTrait, Set};
use jsonrpsee::core::async_trait;
use libgoog::auth::{AccessToken, RefreshToken};
use libgoog::{Credentials, GoogClient};
use std::time::Duration;

use crate::crawler::{CrawlError, CrawlResult};
use crate::oauth;
use crate::state::AppState;
use entities::models::{connection, crawl_queue};
use url::Url;

use super::Connection;

pub struct DriveConnection {
    client: GoogClient,
    user: String,
}

impl DriveConnection {
    pub async fn new(state: &AppState, account: &str) -> anyhow::Result<Self> {
        // Load credentials from db
        let creds = connection::get_by_id(&state.db, &Self::id(), account)
            .await?
            .expect("No credentials matching that id");

        let credentials = Credentials {
            access_token: AccessToken::new(creds.access_token),
            refresh_token: creds.refresh_token.map(RefreshToken::new),
            requested_at: creds.granted_at,
            expires_in: creds.expires_in.map(|d| Duration::from_secs(d as u64)),
        };

        if let Some((client_id, client_secret, _)) = oauth::connection_secret(&Self::id()) {
            let mut client = GoogClient::new(
                libgoog::ClientType::Drive,
                &client_id,
                &client_secret,
                "http://localhost:0",
                credentials,
            )?;

            // Update credentials in database whenever we refresh the token.
            {
                let account = account.to_string();
                let state = state.clone();
                client.set_on_refresh(move |new_creds| {
                    log::debug!("received new credentials");
                    let state = state.clone();
                    let account = account.clone();
                    let new_creds = new_creds.clone();
                    tokio::spawn(async move {
                        if let Ok(Some(conn)) =
                            connection::get_by_id(&state.db, &Self::id(), &account).await
                        {
                            let mut update: connection::ActiveModel = conn.into();
                            update.access_token = Set(new_creds.access_token.secret().to_string());
                            // Refresh tokens are optionally sent
                            if let Some(refresh_token) = new_creds.refresh_token {
                                update.refresh_token =
                                    Set(Some(refresh_token.secret().to_string()));
                            }
                            update.expires_in = Set(new_creds
                                .expires_in
                                .map_or_else(|| None, |dur| Some(dur.as_secs() as i64)));
                            update.granted_at = Set(chrono::Utc::now());
                            let res = update.save(&state.db).await;
                            log::debug!("credentials updated: {:?}", res);
                        }
                    });
                });
            }

            Ok(Self {
                client,
                user: account.to_string(),
            })
        } else {
            Err(anyhow::anyhow!("Connection not supported"))
        }
    }

    pub fn is_indexable_mimetype(&self, mime_type: &str) -> bool {
        mime_type == "application/vnd.google-apps.document"
            || mime_type == "application/vnd.google-apps.presentation"
    }

    pub fn to_url(&self, file_id: &str) -> Url {
        let mut url_base = Url::parse(&format!("api://{}/{}", &Self::id(), file_id))
            .expect("Unable to create base URL");
        let _ = url_base.set_username(&self.user);

        url_base
    }
}

#[async_trait]
impl Connection for DriveConnection {
    fn id() -> String {
        "drive.google.com".to_string()
    }

    fn user(&self) -> String {
        self.user.clone()
    }

    async fn sync(&mut self, state: &AppState) {
        log::debug!("syncing w/ connection");

        // Ignore shortcuts
        let ignore_query = "mimeType != 'application/vnd.google-apps.shortcut'".to_string();

        // stream pages of files from the integration & add them to the crawl queue
        let mut next_page = None;
        let mut num_files = 0;

        // Grab the next page of files
        while let Ok(files) = self
            .client
            .list_files(next_page.clone(), Some(ignore_query.clone()))
            .await
        {
            next_page = files.next_page_token;
            num_files += files.files.len();

            let urls = files
                .files
                .iter()
                .map(|file| self.to_url(&file.id).to_string())
                .collect::<Vec<String>>();

            // Enqueue URIs
            let enqueue_settings = EnqueueSettings {
                crawl_type: CrawlType::Api,
                force_allow: true,
                is_recrawl: true,
            };

            if let Err(err) = crawl_queue::enqueue_all(
                &state.db,
                &urls,
                &[],
                &state.user_settings,
                &enqueue_settings,
                None,
            )
            .await
            {
                log::error!("Unable to enqueue: {}", err.to_string());
            }

            if next_page.is_none() {
                break;
            }
        }

        log::debug!("synced {} files", num_files);
    }

    async fn get(&mut self, uri: &Url) -> anyhow::Result<CrawlResult, CrawlError> {
        let file_id = uri.path().trim_start_matches('/');
        let metadata = match self.client.get_file_metadata(file_id).await {
            Ok(file) => file,
            Err(err) => return Err(CrawlError::FetchError(err.to_string())),
        };

        log::debug!("fetching file {} - {:?}", file_id, metadata);

        // Grab text for supported mimetypes
        let content: String = if self.is_indexable_mimetype(&metadata.mime_type) {
            self.client.download_file(file_id).await.map_or_else(
                |_| "".to_string(),
                |b| {
                    // TODO: Pass through to parsers for spreadsheets/etc.
                    if let Ok(s) = std::str::from_utf8(&b) {
                        s.to_string()
                    } else {
                        "".to_string()
                    }
                },
            )
        } else {
            "".to_string()
        };

        Ok(CrawlResult::new(
            uri,
            Some(metadata.web_view_link),
            &content,
            &metadata.name,
            None,
        ))
    }
}
