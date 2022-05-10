/// Fully provision a domain or domain prefix.
/// 1. Make sure that we have a valid robots.txt for the domain
/// 2. We'll grab a list of unique URLs that have been crawled by the web.archive.org
/// 3. We spin up lots of workers to download the all the data immediately.
/// 4. Index!
///
/// TODO: When a lens directory is created, 2 & 3 can be done by our
/// machines and the pre-processed files can be downloaded without crawling.
use chrono::{Duration, Utc};
use futures::{FutureExt, Stream, StreamExt};
use reqwest::{Client, Error};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::Retry;
use url::Url;

use entities::models::crawl_queue;
use entities::sea_orm::DatabaseConnection;
use shared::config::{Limit, UserSettings};

// Using Internet Archive's CDX because it's faster & more reliable.
const ARCHIVE_CDX_ENDPOINT: &str = "https://web.archive.org/cdx/search/cdx";
const ARCHIVE_WEB_ENDPOINT: &str = "https://web.archive.org/web";

fn create_archive_url(url: &str) -> String {
    // Always try to grab the latest archived crawl
    let date = Utc::now();
    format!(
        "{}/{}000000id_/{}",
        ARCHIVE_WEB_ENDPOINT,
        date.format("%Y%m%d"),
        url
    )
}

type CDXResponse = Result<String, Error>;
pub struct CDXStream {
    pub client: Client,
    pub is_done: bool,
    pub limit: usize,
    pub params: HashMap<String, String>,
    pub next_page_request: Option<Pin<Box<dyn Future<Output = CDXResponse>>>>,
}

impl CDXStream {
    pub fn new(client: &Client, prefix: &str, limit: usize) -> Self {
        let last_year = Utc::now() - Duration::weeks(52);
        let last_year = last_year.format("%Y").to_string();

        // More docs on parameters here:
        // https://github.com/internetarchive/wayback/tree/master/wayback-cdx-server#filtering
        let params: HashMap<String, String> = HashMap::from([
            // TODO: Make this configurable in the lens format?
            ("matchType".into(), "prefix".into()),
            // Only successful pages
            ("filter".into(), "statuscode:200".into()),
            // Only HTML docs
            ("filter".into(), "mimetype:text/html".into()),
            // Remove dupes
            ("collapse".into(), "urlkey".into()),
            // If there are too many URLs, let's us paginate
            ("showResumeKey".into(), "true".into()),
            ("limit".into(), limit.to_string()),
            // Only care about the original URL crawled
            ("fl".into(), "original".into()),
            // Only look at things that have existed within the last year.
            ("from".into(), last_year),
            ("url".into(), prefix.into()),
        ]);

        let client_clone = client.clone();
        CDXStream {
            client: client.clone(),
            limit,
            is_done: false,
            params: params.clone(),
            next_page_request: Some(Box::pin(async move {
                fetch_cdx_page(client_clone, params).await
            })),
        }
    }
}

impl Stream for CDXStream {
    type Item = anyhow::Result<HashSet<String>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let inner = self.get_mut();

        if inner.is_done || inner.next_page_request.is_none() {
            return Poll::Ready(None);
        }

        if let Some(mut future) = inner.next_page_request.take() {
            return match future.poll_unpin(cx) {
                Poll::Ready(res) => match res {
                    Ok(response) => {
                        let mut urls = HashSet::new();
                        let mut resume_key = None;

                        for url in response.split('\n') {
                            if url.is_empty() {
                                continue;
                            }

                            // Text after the limit num is the resume key
                            if urls.len() >= inner.limit {
                                resume_key = Some(url.to_string());
                            } else {
                                urls.insert(url.into());
                            }
                        }

                        if let Some(resume_key) = resume_key {
                            inner.params.insert("resumeKey".into(), resume_key);
                            let client = inner.client.clone();
                            let params = inner.params.clone();
                            let fetch =
                                Box::pin(async move { fetch_cdx_page(client, params).await });
                            inner.next_page_request = Some(fetch);
                        } else {
                            inner.is_done = true;
                        }

                        Poll::Ready(Some(Ok(urls)))
                    }
                    Err(err) => {
                        inner.is_done = true;
                        Poll::Ready(Some(Err(err.into())))
                    }
                },
                Poll::Pending => {
                    inner.next_page_request = Some(future);
                    Poll::Pending
                }
            };
        }

        Poll::Pending
    }
}

async fn fetch_cdx_page(client: Client, params: HashMap<String, String>) -> CDXResponse {
    let retry_strat = ExponentialBackoff::from_millis(1000).take(3);
    // If we're hitting the CDX endpoint too fast, wait a little bit before retrying.
    Retry::spawn(retry_strat, || async {
        let req = client.get(ARCHIVE_CDX_ENDPOINT).query(&params);
        let resp = req.send().await;
        match resp {
            Ok(resp) => resp.text().await,
            Err(err) => Err(err),
        }
    })
    .await
}

/// Bootstraps a URL prefix by grabbing all the archived URLs from the past year
/// from the Internet Archive. We then crawl their archived stuff as fast as possible
/// locally to bring the index up to date.
pub async fn bootstrap(
    db: &DatabaseConnection,
    settings: &UserSettings,
    url: &str,
) -> anyhow::Result<usize> {
    // Check for valid URL and normalize it.
    let url = Url::parse(url)?;

    let client = reqwest::Client::new();
    let mut stream = CDXStream::new(&client, &url.to_string(), 1337);

    let mut count: usize = 0;
    let overrides = crawl_queue::EnqueueSettings {
        skip_blocklist: true,
        crawl_type: crawl_queue::CrawlType::Bootstrap,
    };

    // Stream pages of URLs from the CDX server & add them to our crawl queue.
    while let Some(result) = stream.next().await {
        if let Ok(urls) = result {
            // Some URLs/domains might have a crazy amount of URLs, so we limit
            // how much data to bootstrap based on user settings
            if let Limit::Finite(limit) = settings.domain_crawl_limit {
                if count > limit as usize {
                    return Ok(count);
                }
            }

            // Add URLs to crawl queue
            for url in urls.iter() {
                let archive_url = create_archive_url(url);
                match crawl_queue::enqueue(db, &archive_url, settings, &overrides).await? {
                    Some(_) => {}
                    None => count += 1,
                }
            }
        }
    }

    Ok(count)
}

#[cfg(test)]
mod test {
    use super::{bootstrap, CDXStream};
    use entities::models::crawl_queue;
    use entities::test::setup_test_db;
    use futures::StreamExt;

    use shared::config::{Limit, UserSettings};

    // These tests are ignored since they hit a 3rd party service and we don't
    // want them to be run everytime in CI
    #[tokio::test]
    #[ignore]
    async fn test_bootstrap() {
        let db = setup_test_db().await;
        let settings = UserSettings {
            domain_crawl_limit: Limit::Finite(2),
            ..Default::default()
        };

        let res = bootstrap(&db, &settings, "https://roll20.net/compendium/dnd5e").await;
        assert_eq!(res.unwrap(), 2269);
        let num_queue = crawl_queue::num_queued(&db, crawl_queue::CrawlStatus::Queued)
            .await
            .unwrap();
        assert_eq!(num_queue, 2269);
    }

    #[tokio::test]
    #[ignore]
    async fn test_stream() {
        let client = reqwest::Client::new();
        let mut stream = CDXStream::new(&client, "https://roll20.net/compendium/dnd5e", 100);

        let mut all_urls = Vec::new();
        while let Some(results) = stream.next().await {
            let page = results.unwrap();
            all_urls.extend(page.clone());
        }

        assert_eq!(all_urls.len(), 2275);
    }
}
