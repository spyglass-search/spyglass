use entities::models::crawl_queue::{CrawlType, EnqueueSettings};
use entities::models::tag::{TagPair, TagType};
use jsonrpsee::core::async_trait;
use libgoog::GoogClient;

use crate::crawler::{CrawlError, CrawlResult};
use crate::oauth;
use crate::state::AppState;
use entities::models::crawl_queue;
use url::Url;

use super::{handle_sync_credentials, load_credentials, Connection};

pub struct GCalConnection {
    client: GoogClient,
    user: String,
}

impl GCalConnection {
    pub async fn new(state: &AppState, account: &str) -> anyhow::Result<Self> {
        let credentials = load_credentials(&state.db, &Self::id(), account).await?;
        let (client_id, client_secret, _) =
            oauth::connection_secret(&Self::id()).expect("Connection not supported");

        let mut client = GoogClient::new(
            libgoog::ClientType::Calendar,
            &client_id,
            &client_secret,
            "http://localhost:0",
            credentials,
        )?;

        handle_sync_credentials(&mut client, &state.db, &Self::id(), account).await;

        Ok(Self {
            client,
            user: account.to_string(),
        })
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

    fn default_tags(&self) -> Vec<TagPair> {
        vec![
            (TagType::Source, Self::id()),
            (TagType::Lens, "Calendar".into()),
        ]
    }

    async fn sync(&mut self, state: &AppState) {
        log::debug!("syncing w/ connection: {}", &Self::id());

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
                    let mut tags: Vec<TagPair> = Vec::new();
                    for attendee in &event.attendees {
                        if attendee.is_organizer {
                            tags.push((TagType::Owner, attendee.email.clone()));
                        } else {
                            tags.push((TagType::SharedWith, attendee.email.clone()));
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
                    let mut crawl_result =
                        CrawlResult::new(uri, Some(event.html_link), &content, &title, None);
                    crawl_result.tags = tags;

                    Ok(crawl_result)
                }
                Err(err) => Err(CrawlError::FetchError(err.to_string())),
            };
        }

        Err(CrawlError::FetchError("Invalid URL".to_string()))
    }
}
