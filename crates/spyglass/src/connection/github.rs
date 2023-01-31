use entities::models::tag::{TagPair, TagType, TagValue};
use jsonrpsee::core::async_trait;
use libgithub::types::{Issue, Repo};
use libgithub::GithubClient;
use strum_macros::{Display, EnumString};
use url::Url;

use super::credentials::connection_secret;
use super::{handle_sync_credentials, load_credentials, Connection};
use crate::crawler::{CrawlError, CrawlResult};
use crate::documents::process_crawl_results;
use crate::search::Searcher;
use crate::state::AppState;

const BUFFER_SYNC_SIZE: usize = 500;

#[derive(Display, EnumString)]
pub enum GithubDocTypes {
    #[strum(serialize = "issue")]
    Issue,
    #[strum(serialize = "repository")]
    Repository,
}

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

    async fn sync_issues(&mut self, state: &AppState) {
        let mut page = Some(1);
        let mut total_synced = 0;
        let mut buffer = Vec::new();

        while let Ok(resp) = self.client.list_issues(page).await {
            page = resp.next_page;
            total_synced += resp.result.len();
            buffer.extend(resp.result);

            if buffer.len() > BUFFER_SYNC_SIZE || page.is_none() {
                // Add to database
                let mut crawls = Vec::new();
                for issue in &buffer {
                    let api_url = self.to_url(&issue.url).expect("unable to create url");
                    log::debug!("issue: {}", issue.title);
                    crawls.push(issue_to_crawl(&api_url, issue));
                }

                if let Err(err) = process_crawl_results(state, &crawls, &self.default_tags()).await
                {
                    log::error!("Unable to add issue: {}", err);
                }

                if let Err(err) = Searcher::save(state).await {
                    log::error!("Unable to save repos: {}", err);
                }

                buffer.clear();
            }

            if page.is_none() {
                break;
            }
        }

        log::debug!("synced {} issues", total_synced);
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
                let mut crawls = Vec::new();
                for res in &buffer {
                    let api_url = self.to_url(&res.url).expect("unable to create url");
                    log::debug!("repo: {} - {}", res.full_name, api_url.to_string());
                    crawls.push(repo_to_crawl(&api_url, res));
                }

                if let Err(err) = process_crawl_results(state, &crawls, &self.default_tags()).await
                {
                    log::error!("Unable to add repo: {}", err);
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

    async fn sync_starred(&mut self, state: &AppState) {
        let mut page = Some(1);
        let mut total_synced = 0;
        let mut buffer = Vec::new();

        let mut tags = self.default_tags().clone();
        tags.push((TagType::Favorited, TagValue::Favorited.to_string()));

        while let Ok(resp) = self.client.list_starred(page).await {
            page = resp.next_page;
            total_synced += resp.result.len();
            buffer.extend(resp.result);
            // Save to DB when we've reached a limit or there are no more pages.
            if buffer.len() > BUFFER_SYNC_SIZE || page.is_none() {
                // Add to database
                let mut crawls = Vec::new();
                for res in &buffer {
                    let api_url = self.to_url(&res.url).expect("unable to create url");
                    log::debug!("starred: {} - {}", res.full_name, api_url.to_string());
                    crawls.push(repo_to_crawl(&api_url, res));
                }

                if let Err(err) = process_crawl_results(state, &crawls, &tags).await {
                    log::error!("Unable to add repo: {}", err);
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
        self.sync_issues(state).await;
        self.sync_repos(state).await;
        self.sync_starred(state).await;
    }

    async fn get(&mut self, uri: &Url) -> anyhow::Result<CrawlResult, CrawlError> {
        let mut uri = uri.clone();
        if uri.scheme() != "api" {
            return Err(CrawlError::Unsupported("Invalid URL".to_string()));
        }

        let _ = uri.set_username("");
        let fetch_uri = uri.to_string().replace("api://", "https://");

        if fetch_uri.contains("/issues/") {
            match self.client.get_issue(&fetch_uri).await {
                Ok(issue) => {
                    let mut issue = issue_to_crawl(&uri, &issue);
                    issue.tags.extend(self.default_tags());
                    Ok(issue)
                }
                Err(err) => Err(CrawlError::FetchError(err.to_string())),
            }
        } else {
            match self.client.get_repo(&fetch_uri).await {
                Ok(repo) => {
                    let mut repo = repo_to_crawl(&uri, &repo);
                    repo.tags.extend(self.default_tags());
                    Ok(repo)
                }
                Err(err) => Err(CrawlError::FetchError(err.to_string())),
            }
        }
    }
}

fn issue_to_crawl(api_url: &Url, issue: &Issue) -> CrawlResult {
    let mut result = CrawlResult::new(
        api_url,
        Some(issue.html_url.clone()),
        &issue.to_text(),
        &issue.title,
        None,
    );

    result.tags.push((TagType::Owner, issue.user.login.clone()));
    result
        .tags
        .push((TagType::Repository, issue.repository.full_name.clone()));
    result
        .tags
        .push((TagType::Type, GithubDocTypes::Issue.to_string()));

    result
}

fn repo_to_crawl(api_url: &Url, repo: &Repo) -> CrawlResult {
    let mut result = CrawlResult::new(
        api_url,
        Some(repo.html_url.clone()),
        &repo.description.clone().unwrap_or_default(),
        &repo.full_name,
        None,
    );

    result.tags.push((TagType::Owner, repo.owner.login.clone()));
    result
        .tags
        .push((TagType::Type, GithubDocTypes::Repository.to_string()));

    result
}
