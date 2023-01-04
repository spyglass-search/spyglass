use reqwest::header;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use strum_macros::{AsRefStr, Display};

const ENDPOINT: &str = "https://api.mixpanel.com/track";
const PROJECT_TOKEN: &str = "51d84766a0838458d63998f1e4566d3b";

#[derive(Clone)]
pub struct Metrics {
    client: reqwest::Client,
    disabled: bool,
    uid: String,
}

#[derive(AsRefStr, Display)]
pub enum Event {
    #[strum(serialize = "search")]
    Search { filters: Vec<String> },
    #[strum(serialize = "search_result")]
    SearchResult { domain: String },
    #[strum(serialize = "update_check")]
    UpdateCheck { current_version: String },
}

#[derive(Serialize)]
struct EventProps {
    event: String,
    properties: HashMap<String, Value>,
}

impl EventProps {
    pub fn new(uid: &str, event: &str) -> Self {
        let mut properties: HashMap<String, Value> = HashMap::new();
        properties.insert("token".into(), PROJECT_TOKEN.into());
        properties.insert("time".into(), chrono::Utc::now().timestamp().into());
        properties.insert("distinct_id".into(), uid.into());
        properties.insert(
            "$insert_id".into(),
            uuid::Uuid::new_v4().as_hyphenated().to_string().into(),
        );

        EventProps {
            event: event.to_string(),
            properties,
        }
    }
}

impl Metrics {
    pub fn new(uid: &str, disabled: bool) -> Self {
        let mut headers = header::HeaderMap::new();
        headers.insert("accept", header::HeaderValue::from_static("text/plain"));
        headers.insert(
            "content-type",
            header::HeaderValue::from_static("application/json"),
        );

        let client = reqwest::ClientBuilder::new()
            .default_headers(headers)
            .build()
            .expect("Unable to create HTTP client");

        Self {
            client,
            disabled,
            uid: uid.to_owned(),
        }
    }

    pub async fn track(&self, event: Event) -> anyhow::Result<()> {
        // nothing to do if telemetry is disabled.
        if self.disabled {
            return Ok(());
        }

        let mut data = EventProps::new(&self.uid, event.as_ref());
        match &event {
            Event::Search { filters } => {
                data.properties
                    .insert("filter".into(), filters.to_owned().into());
            }
            Event::SearchResult { domain } => {
                data.properties
                    .insert("domain".into(), domain.as_str().into());
            }
            Event::UpdateCheck { current_version } => {
                data.properties
                    .insert("current_version".into(), current_version.as_str().into());
            }
        }

        let _ = self.client.post(ENDPOINT).json(&vec![data]).send().await?;

        Ok(())
    }
}
