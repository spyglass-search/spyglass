use entities::models::tag::{TagPair, TagType};
use jsonrpsee::core::async_trait;
use libgithub::GithubClient;
use url::Url;

use super::credentials::connection_secret;
use super::{handle_sync_credentials, load_credentials, Connection};
use crate::crawler::{CrawlError, CrawlResult};
use crate::search::Searcher;
use crate::state::AppState;
use crate::task::worker::add_document_and_tags;

const BUFFER_SYNC_SIZE: usize = 500;

pub struct GithubConnection {
    client: GithubClient,
    user: String,
}

impl GithubConnection {
    pub async fn new(state: &AppState, account: &str) -> anyhow::Result<Self> {
        let credentials = load_credentials(&state.db, &Self::id(), account).await?;
        let (client_id, client_secret, _) =
            connection_secret(&Self::id()).expect("Connection not supported");

        let mut client = GithubClient::new(
            &client_id,
            &client_secret,
            "http://127.0.0.1:0",
            credentials,
        )?;

        handle_sync_credentials(&mut client, &state.db, &Self::id(), account).await;

        Ok(Self {
            client,
            user: account.to_string(),
        })
    }

    pub fn to_url(&self, url: &str) -> anyhow::Result<Url> {
        let url = url.replace("https", "api");
        let mut url_base = Url::parse(&url)?;
        let _ = url_base.set_username(&self.user);
        Ok(url_base)
    }

    async fn sync_repos(&mut self, state: &AppState) {
        let mut page = Some(1);
        let mut total_synced = 0;
        let mut buffer = Vec::new();

        while let Ok(resp) = self.client.list_repos(page).await {
            page = resp.next_page;
            total_synced += resp.result.len();
            buffer.extend(resp.result);
            // Save to DB when we've reached a limit or there are no more pages.
            if buffer.len() > BUFFER_SYNC_SIZE || page.is_none() {
                // Add to database
                for res in &buffer {
                    let api_url = self.to_url(&res.url).expect("unable to create url");
                    log::debug!("repo: {} - {}", res.full_name, api_url.to_string());
                    let result = CrawlResult::new(
                        &api_url,
                        Some(res.html_url.clone()),
                        &res.description.clone().unwrap_or_default(),
                        &res.full_name,
                        None,
                    );

                    if let Err(err) =
                        add_document_and_tags(state, &result, &self.default_tags()).await
                    {
                        log::error!("Unable to add repo: {}", err);
                    }
                }

                if let Err(err) = Searcher::save(state).await {
                    log::error!("Unable to save repos: {}", err);
                }

                // clear buffer
                buffer.clear()
            }

            if page.is_none() {
                break;
            }
        }

        log::debug!("synced {} repos", total_synced);
    }
}

#[async_trait]
impl Connection for GithubConnection {
    fn id() -> String {
        "api.github.com".to_string()
    }

    fn user(&self) -> String {
        self.user.clone()
    }

    fn default_tags(&self) -> Vec<TagPair> {
        vec![
            (TagType::Source, Self::id()),
            (TagType::Lens, "GitHub".into()),
        ]
    }

    async fn sync(&mut self, state: &AppState) {
        log::debug!("syncing w/ connection: {}", &Self::id());
        self.sync_repos(state).await;
    }

    async fn get(&mut self, _uri: &Url) -> anyhow::Result<CrawlResult, CrawlError> {
        Ok(CrawlResult::default())
    }
}
