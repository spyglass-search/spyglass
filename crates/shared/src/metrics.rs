use reqwest::header;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use strum_macros::{AsRefStr, Display};

use crate::OS_STR;

#[allow(dead_code)]
const ENDPOINT: &str = "https://api.mixpanel.com/track";
const PROJECT_TOKEN: &str = "51d84766a0838458d63998f1e4566d3b";

#[allow(dead_code)]
#[derive(Clone)]
pub struct Metrics {
    client: reqwest::Client,
    disabled: bool,
    uid: String,
}

#[derive(AsRefStr, Display)]
pub enum Event {
    #[strum(serialize = "authorize_connection")]
    AuthorizeConnection { api_id: String },
    #[strum(serialize = "install_lens")]
    InstallLens { lens: String },
    #[strum(serialize = "install_lens_from_url")]
    InstallLensFromUrl { lens: String },
    #[strum(serialize = "spyglass_started")]
    SpyglassStarted,
    #[strum(serialize = "local_file_scanning_enabled")]
    LocalFileScanningEnabled,
    #[strum(serialize = "local_file_scanning_disabled")]
    LocalFileScanningDisabled,
    #[strum(serialize = "update_lens")]
    UpdateLens { lens: String },
    #[strum(serialize = "search")]
    Search { filters: Vec<String> },
    #[strum(serialize = "search_result")]
    SearchResult {
        num_results: usize,
        num_docs: u64,
        term_count: i32,
        domains: Vec<String>,
        wall_time_ms: u64,
    },
    #[strum(serialize = "update_check")]
    UpdateCheck { current_version: String },
    #[strum(serialize = "wizard_finished")]
    WizardFinished { current_version: String },
    #[strum(serialize = "wizard_closed")]
    WizardClosed { current_version: String },
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
        properties.insert("$os".into(), OS_STR.into());
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

    pub async fn track(&self, event: Event) {
        // nothing to do if telemetry is disabled.
        if self.disabled {
            return;
        }

        let mut data = EventProps::new(&self.uid, event.as_ref());
        match &event {
            Event::AuthorizeConnection { api_id } => {
                data.properties
                    .insert("api_id".into(), api_id.to_owned().into());
            }
            Event::InstallLens { lens } => {
                data.properties
                    .insert("lens".into(), lens.to_owned().into());
            }
            Event::InstallLensFromUrl { lens } => {
                data.properties
                    .insert("lens".into(), lens.to_owned().into());
            }
            Event::UpdateLens { lens } => {
                data.properties
                    .insert("lens".into(), lens.to_owned().into());
            }
            Event::Search { filters } => {
                data.properties
                    .insert("filter".into(), filters.to_owned().into());
            }
            Event::SearchResult {
                num_results,
                num_docs,
                term_count,
                domains,
                wall_time_ms,
            } => {
                data.properties
                    .insert("num_results".into(), num_results.to_owned().into());
                data.properties
                    .insert("num_docs".into(), num_docs.to_owned().into());
                data.properties
                    .insert("term_count".into(), term_count.to_owned().into());
                data.properties
                    .insert("domains".into(), domains.to_owned().into());
                data.properties
                    .insert("wall_time_ms".into(), wall_time_ms.to_owned().into());
            }
            Event::WizardFinished { current_version }
            | Event::UpdateCheck { current_version }
            | Event::WizardClosed { current_version } => {
                data.properties
                    .insert("current_version".into(), current_version.as_str().into());
            }
            Event::LocalFileScanningEnabled
            | Event::LocalFileScanningDisabled
            | Event::SpyglassStarted => {
                //noop
            }
        }

        #[cfg(not(debug_assertions))]
        let _ = self.client.post(ENDPOINT).json(&vec![data]).send().await;
    }
}
