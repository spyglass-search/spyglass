use entities::models::crawl_queue::{CrawlType, EnqueueSettings};
use entities::sea_orm::{ActiveModelTrait, Set};
use jsonrpsee::core::async_trait;
use libgoog::auth::{AccessToken, RefreshToken};
use libgoog::{Credentials, GoogClient};
use std::time::Duration;

use crate::crawler::CrawlResult;
use crate::oauth;
use crate::state::AppState;
use entities::models::{connection, crawl_queue};
use url::Url;

#[async_trait]
pub trait Connection {
    fn id() -> String
    where
        Self: Sized;
    /// Add URIs to crawl queue that are new/updated & remove ones that have
    /// been deleted.
    async fn sync(&mut self, state: &AppState);

    /// Get raw data for a URI
    async fn get(&mut self, uri: &Url) -> anyhow::Result<Option<CrawlResult>>;
}

pub struct DriveConnection {
    client: GoogClient,
}

impl DriveConnection {
    pub async fn new(state: &AppState) -> anyhow::Result<Self> {
        // Load credentials from db
        let creds = connection::get_by_id(&state.db, &Self::id())
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
                &client_id,
                &client_secret,
                "http://localhost:0",
                credentials,
            )?;

            // Update credentials in database whenever we refresh the token.
            {
                let state = state.clone();
                client.set_on_refresh(move |new_creds| {
                    log::debug!("received new credentials");
                    let state = state.clone();
                    let new_creds = new_creds.clone();
                    tokio::spawn(async move {
                        if let Ok(Some(conn)) = connection::get_by_id(&state.db, &Self::id()).await
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

            Ok(Self { client })
        } else {
            Err(anyhow::anyhow!("Connection not supported"))
        }
    }

    pub fn is_indexable_mimetype(&self, mime_type: &str) -> bool {
        mime_type == "application/vnd.google-apps.document"
            || mime_type == "application/vnd.google-apps.presentation"
    }
}

#[async_trait]
impl Connection for DriveConnection {
    fn id() -> String {
        "drive.google.com".to_string()
    }

    async fn sync(&mut self, state: &AppState) {
        log::debug!("syncing w/ connection");

        // Ignore shortcuts
        let ignore_query = "mimeType != 'application/vnd.google-apps.shortcut'".to_string();

        // stream pages of files from the integration & add them to the crawl queue
        let mut next_page = None;
        let mut num_files = 0;

        let url_base =
            Url::parse(&format!("api://{}", &Self::id())).expect("Unable to create base URL");

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
                .map(|file| {
                    let mut crawl_url = url_base.clone();
                    crawl_url.set_path(&file.id);
                    crawl_url.to_string()
                })
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

    async fn get(&mut self, uri: &Url) -> anyhow::Result<Option<CrawlResult>> {
        let file_id = uri.path().trim_start_matches('/');
        let metadata = self.client.get_file_metadata(file_id).await?;
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

        Ok(Some(CrawlResult::new(
            uri,
            Some(metadata.web_view_link),
            &content,
            &metadata.name,
            None,
        )))
    }
}
