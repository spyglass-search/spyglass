use reqwest::header;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use strum_macros::{AsRefStr, Display};

#[allow(dead_code)]
const ENDPOINT: &str = "https://api.mixpanel.com/track";
const PROJECT_TOKEN: &str = "fd3c1af155204ebe46ef88dbc7c9e469";

#[allow(dead_code)]
#[derive(Clone)]
pub struct Metrics {
    client: reqwest::Client,
    disabled: bool,
}

#[derive(AsRefStr, Display)]
pub enum WebClientEvent {
    #[strum(serialize = "login")]
    Login,
    #[strum(serialize = "logout")]
    Logout,
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
    pub fn new(disabled: bool) -> Self {
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

        Self { client, disabled }
    }

    #[allow(dead_code)]
    pub async fn track(&self, event: WebClientEvent, uuid: &str) {
        // nothing to do if telemetry is disabled.
        if self.disabled {
            return;
        }

        let data = EventProps::new(uuid, event.as_ref());

        #[cfg(not(debug_assertions))]
        let _ = self.client.post(ENDPOINT).json(&vec![data]).send().await;
    }
}
