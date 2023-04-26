use anyhow::Result;
use chrono::{DateTime, Utc};
use entities::models::{
    connection,
    tag::{TagPair, TagType},
};
use jsonrpsee::core::async_trait;
use libreddit::{
    types::{ApiResponse, Post},
    RedditClient,
};
use strum_macros::{Display, EnumString};
use url::Url;

use super::{
    credentials::connection_secret, handle_sync_credentials, load_credentials, Connection,
};
use crate::{
    crawler::{CrawlError, CrawlResult},
    documents::process_crawl_results,
    state::AppState,
};

pub struct RedditConnection {
    client: RedditClient,
    user: String,
}

const BUFFER_SYNC_SIZE: usize = 500;

#[derive(Display, EnumString)]
enum FavoriteType {
    #[strum(serialize = "saved")]
    Saved,
    #[strum(serialize = "upvoted")]
    Upvoted,
}

/// API id for Reddit connections
pub const API_ID: &str = "oauth.reddit.com";
/// Lens name for indexed content from Reddit
pub const LENS: &str = "Reddit";
pub const TITLE: &str = "Reddit";
pub const DESCRIPTION: &str = "Adds indexing support for Reddit saved/upvoted posts & comments.";

impl RedditConnection {
    pub async fn new(state: &AppState, account: &str) -> Result<Self> {
        let credentials = load_credentials(&state.db, &Self::id(), account).await?;
        let (client_id, client_secret, _) =
            connection_secret(&Self::id()).expect("Connection not supported");

        let mut client = RedditClient::new(
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

    pub fn to_url(&self, id: &str) -> Result<Url> {
        let mut url_base = Url::parse("api://oauth.reddit.com/api/info")?;
        url_base.set_query(Some(&format!("id={}", id)));
        let _ = url_base.set_username(&self.user);
        Ok(url_base)
    }

    async fn process_list(
        &mut self,
        state: &AppState,
        buffer: &mut Vec<Post>,
        resp: &ApiResponse<Vec<Post>>,
        tags: &[TagPair],
    ) -> Option<String> {
        let page = resp.after.clone();
        buffer.extend(resp.data.clone());

        if buffer.len() > BUFFER_SYNC_SIZE || page.is_none() {
            // Add to database
            let mut posts = Vec::new();
            for post in buffer.iter() {
                let api_url = self.to_url(&post.name).expect("unable to create url");
                log::debug!("post: {}", api_url);
                posts.push(post_to_crawl(&api_url, post));
            }

            if !posts.is_empty() {
                if let Err(err) = process_crawl_results(state, &posts, tags).await {
                    log::warn!("Unable to add posts: {}", err);
                }

                if let Err(err) = state.index.save().await {
                    log::warn!("Unable to save posts: {}", err);
                }
            }

            buffer.clear();
        }

        page
    }

    async fn sync_saved(&mut self, state: &AppState) -> Result<usize> {
        let mut page = None;
        let mut total_synced = 0;
        let mut buffer = Vec::new();

        let mut tags = self.default_tags().clone();
        tags.push((TagType::Favorited, FavoriteType::Saved.to_string()));

        loop {
            let resp = self.client.list_saved(page, 100).await?;
            total_synced += resp.data.len();
            page = self.process_list(state, &mut buffer, &resp, &tags).await;

            if page.is_none() {
                break;
            }
        }

        Ok(total_synced)
    }

    async fn sync_upvoted(&mut self, state: &AppState) -> Result<usize> {
        let mut page = None;
        let mut total_synced = 0;
        let mut buffer = Vec::new();

        let mut tags = self.default_tags().clone();
        tags.push((TagType::Favorited, FavoriteType::Upvoted.to_string()));

        loop {
            let resp = self.client.list_upvoted(page, 100).await?;
            total_synced += resp.data.len();
            page = self.process_list(state, &mut buffer, &resp, &tags).await;

            if page.is_none() {
                break;
            }
        }

        Ok(total_synced)
    }
}

#[async_trait]
impl Connection for RedditConnection {
    fn id() -> String {
        API_ID.to_string()
    }

    fn user(&self) -> String {
        self.user.clone()
    }

    fn default_tags(&self) -> Vec<TagPair> {
        vec![(TagType::Source, Self::id()), (TagType::Lens, LENS.into())]
    }

    async fn sync(&mut self, state: &AppState, _last_synced_at: Option<DateTime<Utc>>) {
        log::debug!("syncing w/ connection: {}", &Self::id());
        let _ = connection::set_sync_status(&state.db, &Self::id(), &self.user, true).await;

        match self.sync_saved(state).await {
            Ok(num_synced) => log::info!("synced {num_synced} saved posts"),
            Err(err) => log::warn!("Unable to sync saved posts: {err}"),
        }

        match self.sync_upvoted(state).await {
            Ok(num_synced) => log::info!("synced {num_synced} upvoted posts"),
            Err(err) => log::warn!("Unable to sync upvoted posts: {}", err),
        }

        let _ = connection::set_sync_status(&state.db, &Self::id(), &self.user, false).await;
    }

    async fn get(&mut self, _: &Url) -> anyhow::Result<CrawlResult, CrawlError> {
        Err(CrawlError::Other("not supported".into()))
    }
}

fn post_to_crawl(api_url: &Url, post: &Post) -> CrawlResult {
    let open_url = format!("https://www.reddit.com{}", post.permalink);
    let mut tags: Vec<TagPair> = Vec::new();

    tags.push((TagType::Owner, post.author.clone()));
    tags.push((TagType::Other("subreddit".into()), post.subreddit.clone()));

    // Comment
    let mut result =
        if let (Some(title), Some(body)) = (post.link_title.as_deref(), post.body.as_deref()) {
            tags.push((TagType::Type, "Comment".into()));
            CrawlResult::new(api_url, Some(open_url), body, title, None)
        } else {
            let content = if post.is_self {
                tags.push((TagType::Type, "Post".into()));
                post.selftext.clone()
            } else {
                tags.push((TagType::Type, "Link".into()));
                post.url.clone()
            };

            let title = post.title.as_deref().unwrap_or_default();

            CrawlResult::new(api_url, Some(open_url), &content, title, None)
        };

    result.tags.extend(tags);
    result
}
