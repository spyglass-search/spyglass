use reqwest::{Client, Response};
use url::Url;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// A wrapper around reqwest that for HTTP related queries that handles retries,
/// downgrading from HTTPS -> HTTP, 429 too many requests, etc.
#[derive(Clone, Debug)]
pub struct HTTPClient {
    client: Client
}

impl HTTPClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            // TODO: Make configurable
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Unable to create reqwest client");

        HTTPClient { client }
    }

    pub async fn head(&self, url: &Url) -> anyhow::Result<Response> {
        todo!()
    }

    pub async fn get(&self, url: &Url) -> anyhow::Result<Response> {
        let mut url = url.clone();
        url.set_scheme("https").unwrap();

        todo!()
    }
}