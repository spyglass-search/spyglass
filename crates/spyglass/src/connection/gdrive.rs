use entities::models::connection;
use entities::models::crawl_queue::{self, CrawlType, EnqueueSettings};
use entities::models::tag::{TagPair, TagType, TagValue};
use jsonrpsee::core::async_trait;
use libgoog::GoogClient;
use url::Url;

use super::credentials::connection_secret;
use crate::crawler::{CrawlError, CrawlResult};
use crate::documents::process_crawl_results;
use crate::state::AppState;

use super::{handle_sync_credentials, load_credentials, Connection};

const BUFFER_SYNC_SIZE: usize = 500;

/// The api id for google drive connections
pub const API_ID: &str = "drive.google.com";
/// The lens name for indexed documents from google drive
pub const LENS: &str = "GDrive";
/// The title for google drive connections
pub const TITLE: &str = "Google Drive";
/// The description for google drive connections
pub const DESCRIPTION: &str = "Adds indexing support for Google drive. This will allow you to search for through documents, spreadsheets, and presentations.";

pub struct DriveConnection {
    client: GoogClient,
    user: String,
}

impl DriveConnection {
    pub async fn new(state: &AppState, account: &str) -> anyhow::Result<Self> {
        let credentials = load_credentials(&state.db, &Self::id(), account)
            .await
            .expect("No credentials matching that id");

        let (client_id, client_secret, _) =
            connection_secret(&Self::id()).expect("Connection not supported");

        let mut client = GoogClient::new(
            libgoog::ClientType::Drive,
            &client_id,
            &client_secret,
            "http://localhost:0",
            credentials,
        )?;

        // Update credentials in database whenever we refresh the token.
        handle_sync_credentials(&mut client, &state.db, &Self::id(), account).await;

        Ok(Self {
            client,
            user: account.to_string(),
        })
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
        API_ID.to_string()
    }

    fn user(&self) -> String {
        self.user.clone()
    }

    fn default_tags(&self) -> Vec<TagPair> {
        vec![(TagType::Source, Self::id()), (TagType::Lens, LENS.into())]
    }

    async fn sync(&mut self, state: &AppState) {
        log::debug!("syncing w/ connection");
        let _ = connection::set_sync_status(&state.db, &Self::id(), &self.user, true).await;

        // Ignore shortcuts
        let ignore_query = "mimeType != 'application/vnd.google-apps.shortcut'".to_string();

        // stream pages of files from the integration & add them to the crawl queue
        let mut next_page = None;
        let mut num_files = 0;
        let mut buffer = Vec::new();

        // Grab the next page of files
        while let Ok(resp) = self
            .client
            .list_files(next_page.clone(), Some(ignore_query.clone()))
            .await
        {
            next_page = resp.next_page_token;
            num_files += resp.files.len();
            buffer.extend(resp.files);

            if buffer.len() > BUFFER_SYNC_SIZE || next_page.is_none() {
                let mut crawls = Vec::new();
                let mut to_download = Vec::new();

                for file in &buffer {
                    let api_uri = self.to_url(&file.id);
                    if let Ok(metadata) = self.client.get_file_metadata(&file.id).await {
                        log::debug!("file: {} - {}", metadata.name, metadata.mime_type);
                        crawls.push(file_to_crawl(&api_uri, &metadata, None));
                        if self.is_indexable_mimetype(&metadata.mime_type) {
                            to_download.push(api_uri.to_string());
                        }
                    }
                }

                // Add to index
                if let Err(err) = process_crawl_results(state, &crawls, &self.default_tags()).await
                {
                    log::error!("Unable to add files: {}", err);
                } else {
                    log::debug!("synced buffer");
                }

                // Enqueue the ones we want to download & index the content
                let enqueue_settings = EnqueueSettings {
                    crawl_type: CrawlType::Api,
                    tags: self.default_tags(),
                    force_allow: true,
                    is_recrawl: true,
                };

                if let Err(err) = crawl_queue::enqueue_all(
                    &state.db,
                    &to_download,
                    &[],
                    &state.user_settings,
                    &enqueue_settings,
                    None,
                )
                .await
                {
                    log::error!("Unable to enqueue: {}", err.to_string());
                }

                buffer.clear();
            }

            if next_page.is_none() {
                break;
            }
        }

        let _ = connection::set_sync_status(&state.db, &Self::id(), &self.user, false).await;
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
        let content: Option<String> = if self.is_indexable_mimetype(&metadata.mime_type) {
            self.client.download_file(file_id).await.map_or_else(
                |_| None,
                |b| {
                    // TODO: Pass through to parsers for spreadsheets/etc.
                    if let Ok(s) = std::str::from_utf8(&b) {
                        Some(s.to_string())
                    } else {
                        None
                    }
                },
            )
        } else {
            None
        };

        // Extract and apply tags to crawl result.
        Ok(file_to_crawl(uri, &metadata, content))
    }
}

fn file_to_crawl(
    api_url: &Url,
    file: &libgoog::types::File,
    content: Option<String>,
) -> CrawlResult {
    let mut result = CrawlResult::new(
        api_url,
        Some(file.web_view_link.clone()),
        &content.unwrap_or(file.description.clone()),
        &file.name.clone(),
        Some(file.description.clone()),
    );

    for owner in &file.owners {
        let name = owner
            .email_address
            .clone()
            .unwrap_or(owner.display_name.clone());
        result.tags.push((TagType::Owner, name.clone()));
    }

    result
        .tags
        .push((TagType::MimeType, file.mime_type.clone()));
    if file.starred {
        result
            .tags
            .push((TagType::Favorited, TagValue::Favorited.to_string()));
    }

    if file.mime_type == "application/vnd.google-apps.folder" {
        result
            .tags
            .push((TagType::Type, TagValue::Directory.to_string()))
    } else if file.mime_type.starts_with("image/") {
        result
            .tags
            .push((TagType::Type, TagValue::Image.to_string()));
        result
            .tags
            .push((TagType::Type, TagValue::File.to_string()));
    } else {
        result
            .tags
            .push((TagType::Type, TagValue::File.to_string()));
    }

    result
}
