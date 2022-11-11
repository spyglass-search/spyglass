use entities::models::crawl_queue::{CrawlType, EnqueueSettings};
use entities::models::tag::{self, TagType};
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

pub struct GCalConnection {
    client: GoogClient,
    state: AppState,
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

            Ok(Self {
                client,
                state: state.clone(),
                user: account.to_string(),
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

        // Grab the next page of files
        while let Ok(events) = self.client.list_calendar_events("primary", next_page).await {
            next_page = events.next_page_token;
            num_events += events.items.len();

            let urls = events
                .items
                .iter()
                .map(|event| self.to_url("primary", &event.id).to_string())
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

    async fn get(&mut self, uri: &Url) -> anyhow::Result<CrawlResult, CrawlError> {
        if let Some(segments) = uri.path_segments().map(|c| c.collect::<Vec<_>>()) {
            if segments.len() != 2 {
                return Err(CrawlError::FetchError("Invalid GCal API URL".to_string()));
            }

            let calendar_id = segments.first().expect("Should be len 2").to_string();
            let event_id = segments.last().expect("Should be len 2").to_string();

            return match self
                .client
                .get_calendar_event(&calendar_id, &event_id)
                .await
            {
                Ok(event) => {
                    let mut tags = vec![
                        tag::add_or_create(&self.state.db, TagType::Source, &Self::id()).await,
                    ];
                    for attendee in &event.attendees {
                        if attendee.is_organizer {
                            tags.push(
                                tag::add_or_create(&self.state.db, TagType::Owner, &attendee.email)
                                    .await,
                            );
                        } else {
                            tags.push(
                                tag::add_or_create(
                                    &self.state.db,
                                    TagType::SharedWith,
                                    &attendee.email,
                                )
                                .await,
                            );
                        }
                    }

                    let content = if event.attendees.is_empty() {
                        event.description.unwrap_or_default()
                    } else {
                        let attendees = event
                            .attendees
                            .iter()
                            .map(|item| format!("{} <{}>", item.email, item.display_name))
                            .collect::<Vec<String>>()
                            .join(";");

                        format!(
                            "Attendees: {}\n{}",
                            attendees,
                            &event.description.unwrap_or_default()
                        )
                    };
                    let title = format!("{} ({})", &event.summary, event.start.date);
                    Ok(CrawlResult::new(
                        uri,
                        Some(event.html_link),
                        &content,
                        &title,
                        None,
                    ))
                }
                Err(err) => Err(CrawlError::FetchError(err.to_string())),
            };
        }

        Err(CrawlError::FetchError("Invalid URL".to_string()))
    }
}
