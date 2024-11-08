use std::path::PathBuf;
use std::time::Instant;

use crate::crawler::{cache, CrawlResult};
use crate::documents;
use crate::pipeline::collector::CollectionResult;
use crate::pipeline::PipelineContext;
use crate::state::AppState;

use libnetrunner::parser::ParseResult;
use shared::config::LensConfig;
use tokio::task::JoinHandle;

use super::parser::DefaultParser;
use crate::crawler::archive;

/// processes the cache for a lens. The cache is streamed in from the provided path
/// and processed. After the process is complete the cache is deleted
pub async fn process_update_warc(state: AppState, cache_path: PathBuf) {
    let records = archive::read_warc(&cache_path);
    match records {
        Ok(mut record_iter) => {
            let mut record_list: Vec<JoinHandle<Option<CrawlResult>>> = Vec::new();
            for archive_record in record_iter.by_ref().flatten() {
                let result = CollectionResult {
                    content: CrawlResult {
                        content: Some(archive_record.content),
                        url: archive_record.url.clone(),
                        open_url: Some(archive_record.url),
                        ..Default::default()
                    },
                };
                let new_state = state.clone();
                let parser = DefaultParser::new();
                record_list.push(tokio::spawn(async move {
                    let mut context = PipelineContext::new("Cache Pipeline", new_state.clone());

                    let parse_result = parser.parse(&mut context, &result.content).await;

                    match parse_result {
                        Ok(parse_result) => {
                            let crawl_result = parse_result.content;
                            Some(crawl_result)
                        }
                        _ => Option::None,
                    }
                }));

                if record_list.len() >= 500 {
                    let mut results: Vec<CrawlResult> = Vec::new();
                    for task in record_list {
                        if let Ok(Some(result)) = task.await {
                            results.push(result);
                        }
                    }
                    // process_records(&state, &mut results).await;
                    record_list = Vec::new();
                }
            }

            if !record_list.is_empty() {
                let mut results: Vec<CrawlResult> = Vec::new();
                for task in record_list {
                    if let Ok(Some(result)) = task.await {
                        results.push(result);
                    }
                }
                // process_records(&state, &mut results).await;
            }
        }
        Err(error) => {
            log::error!("Got an error reading archive {:?}", error);
        }
    }

    // attempt to remove processed cache file
    let _ = cache::delete_cache(&cache_path);
}

/// processes the cache for a lens. The cache is streamed in from the provided path
/// and processed. After the process is complete the cache is deleted
pub async fn process_update(
    state: AppState,
    lens: &LensConfig,
    cache_path: PathBuf,
    keep_archive: bool,
) {
    let now = Instant::now();
    let mut total_processed = 0;

    let records = archive::read_parsed(&cache_path);
    if let Ok(mut record_iter) = records {
        let mut record_list: Vec<ParseResult> = Vec::new();
        for record in record_iter.by_ref() {
            total_processed += 1;

            record_list.push(record);
            if record_list.len() >= 5000 {
                if let Err(err) = documents::process_records(&state, lens, &mut record_list).await {
                    log::warn!("Unable to process records: {err}");
                }
                record_list = Vec::new();
            }
        }

        if !record_list.is_empty() {
            if let Err(err) = documents::process_records(&state, lens, &mut record_list).await {
                log::warn!("Unable to process records: {err}");
            }
        }
    }

    // attempt to remove processed cache file
    if !keep_archive {
        let _ = cache::delete_cache(&cache_path);
    }

    log::debug!(
        "Processed {} records in {:?}ms",
        total_processed,
        now.elapsed().as_millis()
    );
    state
        .publish_event(&spyglass_rpc::RpcEvent {
            event_type: spyglass_rpc::RpcEventType::LensInstalled,
            payload: lens.name.to_string(),
        })
        .await;
}
