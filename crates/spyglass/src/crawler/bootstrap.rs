/// Fully provision a domain or domain prefix.
/// 1. Make sure that we have a valid robots.txt for the domain
/// 2. We'll grab a list of unique URLs that have been crawled by the web.archive.org
/// 3. We spin up lots of workers to download the all the data immediately.
/// 4. Index!
///
/// TODO: When a lens directory is created, 2 & 3 can be done by our
/// machines and the pre-processed files can be downloaded without crawling.
use chrono::{Duration, Utc};
use reqwest::{Client, Error};
use std::collections::HashSet;
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::Retry;
use url::Url;

use entities::models::crawl_queue;
use entities::sea_orm::DatabaseConnection;
use shared::config::{Limit, UserSettings};

// Using Internet Archive's CDX because it's faster & more reliable.
const ARCHIVE_CDX_ENDPOINT: &str = "https://web.archive.org/cdx/search/cdx";
const ARCHIVE_WEB_ENDPOINT: &str = "https://web.archive.org/web";

pub fn create_archive_url(url: &str) -> String {
    // Always try to grab the latest archived crawl
    let date = Utc::now();
    format!(
        "{}/{}000000id_/{}",
        ARCHIVE_WEB_ENDPOINT,
        date.format("%Y%m%d"),
        url
    )
}

type CDXResumeKey = Option<String>;
type FetchCDXResult = anyhow::Result<(HashSet<String>, CDXResumeKey)>;

async fn fetch_cdx(
    client: &Client,
    prefix: &str,
    limit: usize,
    resume_key: Option<String>,
) -> FetchCDXResult {
    let last_year = Utc::now() - Duration::weeks(52);
    let last_year = last_year.format("%Y").to_string();

    // More docs on parameters here:
    // https://github.com/internetarchive/wayback/tree/master/wayback-cdx-server#filtering
    let mut params: Vec<(String, String)> = vec![
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
    ];

    if let Some(resume) = resume_key {
        params.push(("resumeKey".into(), resume));
    }

    let response = fetch_cdx_page(client, params).await?;

    let mut urls = HashSet::new();
    let mut resume_key = None;

    for url in response.split('\n') {
        if url.is_empty() {
            continue;
        }

        // Text after the limit num is the resume key
        if urls.len() >= limit {
            resume_key = Some(url.to_string());
        } else {
            urls.insert(url.into());
        }
    }

    Ok((urls, resume_key))
}

async fn fetch_cdx_page(
    client: &Client,
    params: Vec<(String, String)>,
) -> anyhow::Result<String, Error> {
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
    let mut resume_key = None;
    let prefix = url.as_str();

    let mut count: usize = 0;
    let overrides = crawl_queue::EnqueueSettings {
        skip_blocklist: true,
        crawl_type: crawl_queue::CrawlType::Bootstrap,
    };

    // Stream pages of URLs from the CDX server & add them to our crawl queue.
    loop {
        log::info!("fetching page from cdx");
        if let Ok((urls, resume)) = fetch_cdx(&client, prefix, 1000, resume_key.clone()).await {
            // Some URLs/domains might have a crazy amount of crawls, so we limit
            // how much data to bootstrap based on user settings
            if let Limit::Finite(limit) = settings.domain_crawl_limit {
                if count > limit as usize {
                    return Ok(count);
                }
            }

            // Add URLs to crawl queue
            log::info!("enqueing {} urls", urls.len());
            let urls: Vec<String> = urls.into_iter().collect();
            crawl_queue::enqueue_all(db, &urls, settings, &overrides).await?;
            count += urls.len();

            if resume.is_none() {
                return Ok(count);
            }

            resume_key = resume;
        } else {
            return Ok(count);
        }
    }
}

#[cfg(test)]
mod test {
    use super::bootstrap;
    use entities::models::crawl_queue;
    use entities::test::setup_test_db;

    use shared::config::{Limit, UserSettings};

    // These tests are ignored since they hit a 3rd party service and we don't
    // want them to be run everytime in CI
    #[tokio::test]
    #[ignore]
    async fn test_bootstrap() {
        let db = setup_test_db().await;
        let settings = UserSettings {
            domain_crawl_limit: Limit::Infinite,
            ..Default::default()
        };

        let res = bootstrap(&db, &settings, "https://roll20.net/compendium/dnd5e").await;
        assert_eq!(res.unwrap(), 1934);
        let num_queue = crawl_queue::num_queued(&db, crawl_queue::CrawlStatus::Queued)
            .await
            .unwrap();
        assert_eq!(num_queue, 1934);
    }
}
