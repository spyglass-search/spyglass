/// Fully provision a domain or domain prefix.
/// 1. Make sure that we have a valid robots.txt for the domain
/// 2. We'll grab a list of unique URLs that have been crawled by the web.archive.org
/// 3. We spin up lots of workers to download the all the data immediately.
/// 4. Index!
///
/// TODO: When a lens directory is created, 2 & 3 can be done by our
/// machines and the pre-processed files can be downloaded without crawling.
use chrono::{Duration, Utc};
use reqwest::StatusCode;
use url::Url;

use entities::models::crawl_queue;
use entities::sea_orm::DatabaseConnection;
use shared::config::UserSettings;

const ARCHIVE_CDX_ENDPOINT: &str = "https://web.archive.org/cdx/search/cdx";
const ARCHIVE_WEB_ENDPOINT: &str = "https://web.archive.org/web";

// http://web.archive.org/cdx/search/cdx?
//      url=roll20.net/compendium/dnd5e
//      &matchType=prefix
//      &filter=statuscode:200
//      &filter=mimetype:text/html
//      &collapse=urlkey
//      &showResumeKey=true
//      &fl=original

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

/// Check CDX index for a list of URLs that match the prefix
async fn fetch_cdx(prefix: &str) -> anyhow::Result<Vec<String>> {
    let client = reqwest::Client::new();

    let last_year = Utc::now() - Duration::weeks(52);
    let last_year = last_year.format("%Y").to_string();

    // Docs on what the CDX server supports:
    // https://github.com/internetarchive/wayback/tree/master/wayback-cdx-server#filtering
    let params = vec![
        // TODO: Make this configurable in the lens format?
        ("matchType", "prefix"),
        // Only successful pages
        ("filter", "statuscode:200"),
        // Only HTML docs
        ("filter", "mimetype:text/html"),
        // Remove dupes
        ("collapse", "urlkey"),
        // If there are too many URLs, let's us paginate
        ("showResumeKey", "true"),
        // Only care about the original URL crawled
        ("fl", "original"),
        // Only look at things that have existed within the last year.
        ("from", &last_year),
        ("url", prefix),
    ];

    let resp = client.get(ARCHIVE_CDX_ENDPOINT).query(&params);
    let resp = resp.send().await?;
    if resp.status() != StatusCode::OK {
        return Ok(Vec::new());
    }

    let content = resp.text().await?;
    Ok(content.split('\n').map(|x| x.to_string()).collect())
}

pub async fn bootstrap(
    db: &DatabaseConnection,
    settings: &UserSettings,
    url: &str,
) -> anyhow::Result<usize> {
    // Check for valid URL and normalize it.
    let url = Url::parse(url)?;

    // Extract URLs
    let urls = fetch_cdx(url.as_str()).await?;

    // Add URLs to crawl queue
    let mut count = 0;
    let overrides = crawl_queue::EnqueueSettings {
        skip_blocklist: true,
    };

    for url in urls.iter() {
        let archive_url = create_archive_url(url);
        match crawl_queue::enqueue(db, &archive_url, settings, &overrides).await? {
            Some(_) => {}
            None => count += 1,
        }
    }

    Ok(count)
}

#[cfg(test)]
mod test {
    use super::{bootstrap, fetch_cdx};

    use crate::models::crawl_queue;
    use crate::test::setup_test_db;

    use shared::config::{Limit, UserSettings};

    #[tokio::test]
    #[ignore]
    async fn test_cdx_query() {
        let res = fetch_cdx("https://roll20.net/compendium/dnd5e")
            .await
            .unwrap();
        assert_eq!(res.len(), 3777);
    }

    #[tokio::test]
    async fn test_bootstrap() {
        let db = setup_test_db().await;
        let settings = UserSettings {
            domain_crawl_limit: Limit::Finite(2),
            ..Default::default()
        };

        let res = bootstrap(&db, &settings, "https://roll20.net/compendium/dnd5e").await;
        assert_eq!(res.unwrap(), 1933);
        let num_queue = crawl_queue::num_queued(&db).await.unwrap();
        assert_eq!(num_queue, 1933);
    }
}
