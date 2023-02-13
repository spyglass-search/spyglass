/// Fully provision a domain or domain prefix.
/// 1. Make sure that we have a valid robots.txt for the domain
/// 2. We'll grab a list of unique URLs that have been crawled by the web.archive.org
/// 3. We spin up lots of workers to download the all the data immediately.
/// 4. Index!
///
/// TODO: When a lens directory is created, 2 & 3 can be done by our
/// machines and the pre-processed files can be downloaded without crawling.
use chrono::Utc;
use entities::models::crawl_queue;
use entities::models::tag::TagType;
use entities::sea_orm::DatabaseConnection;
use libnetrunner::bootstrap::Bootstrapper;
use shared::config::{Config, LensConfig, UserSettings};

use crate::pipeline::PipelineCommand;
use crate::state::AppState;

use super::cache;

// Using Internet Archive's CDX because it's faster & more reliable.
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

/// Bootstraps a lens using cache. If a cache file exists (either already existed or freshly downloaded) a process cache
/// pipeline command will be kicked off. In the case that no cache exists and no cache has ever existed then false is
/// returned.
pub async fn bootstrap_lens_cache(state: &AppState, config: &Config, lens: &LensConfig) -> bool {
    let cache_result = cache::update_cache(state, config, &lens.name).await;
    match cache_result {
        Ok((Some(cache_file), _)) => {
            if let Some(pipeline_tx) = state.pipeline_cmd_tx.lock().await.as_mut() {
                log::debug!("Sending cache task to pipeline");
                let cmd = PipelineCommand::ProcessCache(lens.name.clone(), cache_file);
                if let Err(err) = pipeline_tx.send(cmd).await {
                    log::error!("Unable to send cache task to pipeline {:?}", err);
                }
            }
            true
        }
        Ok((Option::None, Some(_))) => {
            // No new cache, but a cache has been processed in the past
            true
        }
        _ => {
            // Error accessing cache or no cache exists / was ever processed
            false
        }
    }
}

/// Bootstraps a URL prefix by grabbing all the archived URLs from the past year
/// from the Internet Archive. We then crawl their archived stuff as fast as possible
/// locally to bring the index up to date.
pub async fn bootstrap(
    state: &AppState,
    lens: &LensConfig,
    db: &DatabaseConnection,
    settings: &UserSettings,
    pipeline: Option<String>,
) -> anyhow::Result<usize> {
    let mut shutdown_rx = state.shutdown_cmd_tx.lock().await.subscribe();

    let overrides = crawl_queue::EnqueueSettings {
        crawl_type: crawl_queue::CrawlType::Normal,
        tags: vec![(TagType::Lens, lens.name.to_string())],
        ..Default::default()
    };

    log::info!("kicking off bootstrapper");
    let lens_clone = lens.clone();
    let worker = tokio::spawn(async move {
        let client = reqwest::Client::new();
        let mut bootstrapper = Bootstrapper::new(&client);
        bootstrapper.find_urls(&lens_clone).await
    });

    let urls = tokio::select! {
        res = worker => res,
        _ = shutdown_rx.recv() => {
            log::info!("🛑 Shutting down bootstrapper");
            return Ok(0);
        }
    };

    let urls = urls??;
    let count: usize = urls.len();

    // Add URLs to crawl queue
    if count > 0 {
        log::info!("enqueing {} urls", urls.len());
        crawl_queue::enqueue_all(
            db,
            &urls,
            &[lens.clone()],
            settings,
            &overrides,
            pipeline.clone(),
        )
        .await?;
    } else {
        log::info!("found 0 urls for <{}>, is this an error?", &lens.name);
    }

    Ok(count)
}

#[cfg(test)]
mod test {
    use crate::state::AppState;

    use super::bootstrap;
    use entities::models::crawl_queue;
    use entities::test::setup_test_db;

    use shared::config::{LensConfig, Limit, UserSettings};
    // These tests are ignored since they hit a 3rd party service and we don't
    // want them to be run everytime in CI
    #[tokio::test]
    // #[ignore]
    async fn test_bootstrap() {
        let db = setup_test_db().await;
        let state = AppState::builder().with_db(db.clone()).build();

        let settings = UserSettings {
            domain_crawl_limit: Limit::Infinite,
            ..Default::default()
        };

        let mut lens: LensConfig = Default::default();
        lens.urls.push("https://www.wikipedia.org$".to_string());

        let res = bootstrap(&state, &lens, &db, &settings, Option::None)
            .await
            .expect("Unable to bootstrap");
        assert_eq!(res, 1);
        let num_queue = crawl_queue::num_queued(&db, crawl_queue::CrawlStatus::Queued)
            .await
            .expect("Unable to get num_queued");
        assert_eq!(num_queue, 1);
    }
}
