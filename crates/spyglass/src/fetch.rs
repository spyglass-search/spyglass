use http::StatusCode;
use reqwest::{Client, Response};
use url::Url;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

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
        let mut res = self.client.get(url.clone()).send().await;
        if let Err(e) = &res {
            if e.is_request() {
                url.set_scheme("http")
                    .expect("Unable to set scheme to HTTP");
                res = self.client.get(url.clone()).send().await;
            }
        }

        // Handle 429s
        if let Err(e) = &res {
            if let Some(status) = e.status() {
                if status == StatusCode::TOO_MANY_REQUESTS {
                    // Probably overkill, but if this becomes a problem we can revisit
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    res = self.client.get(url).send().await;
                }
            }
        }

        match res {
            Ok(e) => Ok(e),
            Err(e) => Err(anyhow::Error::from(e)),
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
