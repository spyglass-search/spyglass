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

use super::Connection;

pub struct GCalConnection {
    client: GoogClient,
    user: String,
}

impl GCalConnection {
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
                libgoog::ClientType::Calendar,
                &client_id,
                &client_secret,
                "http://localhost:0",
                credentials,
            )?;

            // Update credentials in database whenever we refresh the token.
            {
                let state = state.clone();
                let account = account.to_string();
                client.set_on_refresh(move |new_creds| {
                    log::debug!("received new credentials");
                    let account = account.clone();
                    let state = state.clone();
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

            let user = client.get_user().await?;
            Ok(Self {
                client,
                user: user.email,
            })
        } else {
            Err(anyhow::anyhow!("Connection not supported"))
        }
    }

    pub fn to_url(&self, cal_id: &str, event_id: &str) -> Url {
        let mut url_base = Url::parse(&format!("api://{}/{}/{}", &Self::id(), cal_id, event_id))
            .expect("Unable to create base URL");
        let _ = url_base.set_username(&self.user);

        url_base
    }
}

#[async_trait]
impl Connection for GCalConnection {
    fn id() -> String {
        "calendar.google.com".to_string()
    }

    fn user(&self) -> String {
        self.user.clone()
    }

    async fn sync(&mut self, state: &AppState) {
        log::debug!("syncing w/ connection");

        // stream pages of files from the integration & add them to the crawl queue
        let mut next_page = None;
        let mut num_events = 0;

        let url_base =
            Url::parse(&format!("api://{}", &Self::id())).expect("Unable to create base URL");

        // Grab the next page of files
        while let Ok(events) = self.client.list_calendar_events("primary", next_page).await {
            next_page = events.next_page_token;
            num_events += events.items.len();

            let urls = events
                .items
                .iter()
                .map(|event| {
                    let mut crawl_url = url_base.clone();
                    crawl_url.set_path(&event.id);
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

        log::debug!("synced {} events", num_events);
    }

    async fn get(&mut self, uri: &Url) -> anyhow::Result<Option<CrawlResult>> {
        if let Some(segments) = uri.path_segments().map(|c| c.collect::<Vec<_>>()) {
            if segments.len() != 2 {
                return Ok(None);
            }

            let calendar_id = segments.first().expect("Should be len 2").to_string();
            let event_id = segments.last().expect("Should be len 2").to_string();

            return match self
                .client
                .get_calendar_event(&calendar_id, &event_id)
                .await
            {
                Ok(event) => Ok(Some(CrawlResult::new(
                    uri,
                    Some(event.html_link),
                    &event.description.unwrap_or_default(),
                    &event.summary,
                    None,
                ))),
                Err(err) => Err(anyhow::anyhow!(err.to_string())),
            };
        }

        Ok(None)
    }
}
