use http::StatusCode;
use reqwest::{Client, Response};
use url::Url;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);
const NUM_RETRIES: usize = 3;
const RETRY_WAIT_S: u64 = 10;
const CODE_429_DELAY_S: u64 = 60;

/// A wrapper around reqwest that for HTTP related queries that handles retries,
/// downgrading from HTTPS -> HTTP, 429 too many requests, etc.
#[derive(Clone, Debug)]
pub struct HTTPClient {
    client: Client,
}

impl Default for HTTPClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HTTPClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            // TODO: Make configurable
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Unable to create reqwest client");

        HTTPClient { client }
    }

    pub async fn head(&self, url: &Url) -> anyhow::Result<Response> {
        let mut url = url.clone();
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(anyhow::Error::msg(format!("Invalid HTTP url: {}", url)));
        }

        url.set_scheme("https")
            .expect("Unable to set scheme to HTTPS");
        let mut res = self.client.head(url.clone()).send().await;
        if let Err(e) = &res {
            if e.is_request() {
                url.set_scheme("http")
                    .expect("Unable to set scheme to HTTP");
                res = self.client.head(url).send().await;
            }
        }

        match res {
            Ok(e) => Ok(e),
            Err(e) => Err(anyhow::Error::from(e)),
        }
    }

    pub async fn get(&self, url: &Url) -> anyhow::Result<Response> {
        let mut url = url.clone();
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(anyhow::Error::msg(format!("Invalid HTTP url: {}", url)));
        }

        // Attempt HTTPS first, if that fails switch to HTTP
        url.set_scheme("https")
            .expect("Unable to set scheme to HTTPS");

        let mut res = None;
        // TODO: Clean up this retry loop, it's a little hard to follow.
        for _ in 0..NUM_RETRIES {
            let request = self.client.get(url.clone()).send().await;
            match &request {
                Err(err) => {
                    // Handle 429s
                    if let Some(status) = err.status() {
                        if status == StatusCode::TOO_MANY_REQUESTS || status.as_u16() == 429 {
                            // Probably overkill, but if this becomes a problem we can revisit
                            log::warn!("Making too many requests, slowing down");
                            tokio::time::sleep(tokio::time::Duration::from_secs(CODE_429_DELAY_S)).await;
                        }
                    } else if err.is_request() && url.scheme() == "https" {
                        // Try downgrading to HTTP if we're unable to connect
                        url.set_scheme("http")
                            .expect("Unable to set scheme to HTTP");
                    }

                    res = Some(request);
                }
                Ok(resp) => {
                    if resp.status() == StatusCode::TOO_MANY_REQUESTS {
                        log::warn!("Making too many requests, slowing down");
                        tokio::time::sleep(tokio::time::Duration::from_secs(CODE_429_DELAY_S)).await;
                        res = Some(request);
                    } else if resp.status().is_success() || resp.status().is_client_error() {
                        res = Some(request);
                        break;
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(RETRY_WAIT_S)).await;
        }

        match res {
            Some(Ok(res)) => Ok(res),
            Some(Err(e)) => Err(anyhow::Error::from(e)),
            None => Err(anyhow::Error::msg(format!("Unable to query <{}>", url))),
        }
    }
}

#[cfg(test)]
mod test {
    use super::HTTPClient;
    use url::Url;

    #[tokio::test]
    #[ignore]
    async fn test_http_switch() {
        let client = HTTPClient::new();
        let url = Url::parse("https://paulgraham.com").unwrap();

        let res = client.get(&url).await;
        if res.is_err() {
            dbg!(&res);
        }

        assert!(res.is_ok());
        // Should have switched to HTTP
        let resp = res.unwrap();
        assert_eq!(resp.url().scheme(), "http");
    }
}
