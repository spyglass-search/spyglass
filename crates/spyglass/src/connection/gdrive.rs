use entities::models::crawl_queue;
use entities::models::crawl_queue::{CrawlType, EnqueueSettings};
use entities::models::tag::{TagPair, TagType, TagValue};
use jsonrpsee::core::async_trait;
use libgoog::GoogClient;
use url::Url;

use super::credentials::connection_secret;
use crate::crawler::{CrawlError, CrawlResult};
use crate::state::AppState;

use super::{handle_sync_credentials, load_credentials, Connection};

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
        "drive.google.com".to_string()
    }

    fn user(&self) -> String {
        self.user.clone()
    }

    fn default_tags(&self) -> Vec<TagPair> {
        vec![
            (TagType::Source, Self::id()),
            (TagType::Lens, "GDrive".into()),
        ]
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
                tags: self.default_tags(),
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

        // Extract and apply tags to crawl result.
        let mut tags: Vec<TagPair> = vec![(TagType::MimeType, metadata.mime_type)];
        if metadata.starred {
            tags.push((TagType::Favorited, TagValue::Favorited.as_ref().to_owned()));
        }

        let mut crawl = CrawlResult::new(
            uri,
            Some(metadata.web_view_link),
            &content,
            &metadata.name,
            None,
        );
        crawl.tags = tags;

        Ok(crawl)
    }
}
