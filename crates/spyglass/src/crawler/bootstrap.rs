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

use entities::models::crawl_queue::{self, EnqueueSettings};
use entities::sea_orm::DatabaseConnection;
use shared::config::{LensConfig, UserSettings};

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
    lens: &LensConfig,
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
        crawl_type: crawl_queue::CrawlType::Bootstrap,
        ..Default::default()
    };

    // Stream pages of URLs from the CDX server & add them to our crawl queue.
    loop {
        log::info!("fetching page from cdx");
        if let Ok((urls, resume)) = fetch_cdx(&client, prefix, 1000, resume_key.clone()).await {
            // Add URLs to crawl queue
            log::info!("enqueing {} urls", urls.len());
            let urls: Vec<String> = urls.into_iter().collect();
            crawl_queue::enqueue_all(db, &urls, &[lens.clone()], settings, &overrides).await?;
            count += urls.len();

            if resume.is_none() {
                break;
            }

            resume_key = resume;
        } else {
            break;
        }

        // Add a little delay so our UI thread is able to get a word in.
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // If no URLs were found to be bootstrap, enqueue the seed url. This can happen
    // if its a new site which the Internet Archive has yet to archive
    if count == 0 {
        log::warn!("No URLs found in CDX, adding <{}> as a normal crawl", url);
        crawl_queue::enqueue_all(
            db,
            &[url.to_string()],
            &[],
            settings,
            // No overrides required
            &EnqueueSettings {
                force_allow: true,
                ..Default::default()
            },
        )
        .await?;
        count += 1;
    }

    Ok(count)
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

        let lens = Default::default();
        let res = bootstrap(&lens, &db, &settings, "https://roll20.net/compendium/dnd5e").await;
        assert!(res.unwrap() > 2000);
        let num_queue = crawl_queue::num_queued(&db, crawl_queue::CrawlStatus::Queued)
            .await
            .unwrap();
        assert!(num_queue > 2000);
    }
}
