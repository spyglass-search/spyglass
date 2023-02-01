use entities::models::tag::{TagPair, TagType};
use jsonrpsee::core::async_trait;
use libgoog::types::CalendarEvent;
use libgoog::GoogClient;

use crate::crawler::{CrawlError, CrawlResult};
use crate::documents::process_crawl_results;
use crate::state::AppState;
use url::Url;

use super::credentials::connection_secret;
use super::{handle_sync_credentials, load_credentials, Connection};

/// The api id for google calendar connections
pub const API_ID: &str = "calendar.google.com";
/// The lens name for indexed documents from google calendar
pub const LENS: &str = "Calendar";
/// The title for google calendar connections
pub const TITLE: &str = "Google Calendar";
/// The description for google calendar connections
pub const DESCRIPTION: &str = "Adds indexing support for Google calendar events.";

const BUFFER_SYNC_SIZE: usize = 500;
pub struct GCalConnection {
    client: GoogClient,
    user: String,
}

impl GCalConnection {
    pub async fn new(state: &AppState, account: &str) -> anyhow::Result<Self> {
        let credentials = load_credentials(&state.db, &Self::id(), account).await?;
        let (client_id, client_secret, _) =
            connection_secret(&Self::id()).expect("Connection not supported");

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
        API_ID.to_string()
    }

    fn user(&self) -> String {
        self.user.clone()
    }

    fn default_tags(&self) -> Vec<TagPair> {
        vec![(TagType::Source, Self::id()), (TagType::Lens, LENS.into())]
    }

    async fn sync(&mut self, state: &AppState) {
        log::debug!("syncing w/ connection: {}", &Self::id());

        // stream pages of files from the integration & add them to the crawl queue
        let mut next_page = None;
        let mut num_events = 0;

        let mut buffer = Vec::new();

        // Grab the next page of files
        while let Ok(events) = self.client.list_calendar_events("primary", next_page).await {
            next_page = events.next_page_token;
            num_events += events.items.len();
            buffer.extend(events.items);

            if buffer.len() > BUFFER_SYNC_SIZE || next_page.is_none() {
                let mut events = Vec::new();
                for event in &buffer {
                    let api_uri = self.to_url("primary", &event.id);
                    log::debug!("gcal event: {}", event.summary);
                    events.push(event_to_crawl(&api_uri, event));
                }

                if let Err(err) = process_crawl_results(state, &events, &self.default_tags()).await
                {
                    log::error!("Unable to add gcal events: {}", err);
                }

                buffer.clear();
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
                    let mut event = event_to_crawl(uri, &event);
                    event.tags.extend(self.default_tags());
                    Ok(event)
                }
                Err(err) => Err(CrawlError::FetchError(err.to_string())),
            };
        }

        Err(CrawlError::FetchError("Invalid URL".to_string()))
    }
}

fn event_to_crawl(api_url: &Url, event: &CalendarEvent) -> CrawlResult {
    let mut tags: Vec<TagPair> = Vec::new();
    for attendee in &event.attendees {
        if attendee.is_organizer {
            tags.push((TagType::Owner, attendee.email.clone()));
        } else {
            tags.push((TagType::SharedWith, attendee.email.clone()));
        }
    }

    let content = event.description.clone().unwrap_or_default();
    let title = format!("{} ({})", &event.summary, event.start.date);
    let mut crawl_result = CrawlResult::new(
        api_url,
        Some(event.html_link.clone()),
        &content,
        &title,
        None,
    );
    crawl_result.tags = tags;

    crawl_result
}
