use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::Instant;

use crate::crawler::{CrawlResult, cache};
use crate::pipeline::collector::{CollectionResult, DefaultCollector};
use crate::pipeline::PipelineContext;
use crate::search::Searcher;
use crate::state::AppState;
use crate::task::CrawlTask;

use entities::models::{crawl_queue, indexed_document};
use shared::config::{Config, LensConfig, PipelineConfiguration};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::parser::DefaultParser;
use crate::crawler::archive;
use entities::sea_orm::query::*;
use entities::sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use url::Url;

/// processes the cache for a lens. The cache is streamed in from the provided path
/// and processed. After the process is complete the cache is deleted
pub async fn process_update(state: AppState, cache_path: PathBuf) {
    let records = archive::read(&cache_path);
    match records {
        Ok(mut record_iter) => {
            let mut record_list: Vec<JoinHandle<Option<CrawlResult>>> = Vec::new();
            while let Some(record) = record_iter.next() {
                if let Some(archive_record) = record {
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
                                return Some(crawl_result);
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
                        process_records(&state, &mut results).await;
                        record_list = Vec::new();
                    }
                }
            }

            if record_list.len() > 0 {
                let mut results: Vec<CrawlResult> = Vec::new();
                for task in record_list {
                    if let Ok(Some(result)) = task.await {
                        results.push(result);
                    }
                }
                process_records(&state, &mut results).await;
            }
        }
        Err(error) => {
            log::error!("Got an error reading archive {:?}", error);
        }
    }
    
    // attempt to remove processed cache file
    let _ = cache::delete_cache(&cache_path);
}

// Process a list of crawl results. The following steps will be taken:
// 1. Find all urls that already have been processed in the database
// 2. Remove any documents that already exist from the index
// 3. Add all new results to the index
// 4. Insert all new documents to the indexed document database
async fn process_records(state: &AppState, results: &mut Vec<CrawlResult>) {
    let find = indexed_document::Entity::find();
    let mut condition = Condition::any();

    for result in &mut *results {
        condition = condition.add(indexed_document::Column::Url.eq(result.url.as_str()));
    }
    let existing: Vec<indexed_document::Model> = find
        .filter(condition)
        .all(&state.db)
        .await
        .unwrap_or_default();
    let mut id_map = HashMap::new();

    for model in &existing {
        let _ = id_map.insert(model.url.to_string(), model.doc_id.clone());
        let _ = Searcher::delete_by_id(&state, model.doc_id.as_str()).await;
    }

    let _ = Searcher::save(&state);

    let transaction_rslt = state.db.begin().await;
    match transaction_rslt {
        Ok(transaction) => {
            let mut updates = Vec::new();
            for crawl_result in results {
                if let Some(content) = &crawl_result.content {
                    log::debug!("Cache Pipeline got content");
                    let url = Url::parse(&crawl_result.url).expect("Invalid crawl URL");
                    let url_host = url.host_str().expect("Invalid URL host");

                    // Add document to index
                    let doc_id: Option<String> = {
                        if let Ok(mut index_writer) = state.index.writer.lock() {
                            match Searcher::upsert_document(
                                &mut index_writer,
                                id_map.get(&crawl_result.url).cloned(),
                                &crawl_result.title.clone().unwrap_or_default(),
                                &crawl_result.description.clone().unwrap_or_default(),
                                url_host,
                                url.as_str(),
                                &content,
                            ) {
                                Ok(new_doc_id) => Some(new_doc_id),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    };

                    if let Some(new_id) = doc_id {
                        if id_map.contains_key(&new_id) == false {
                            let update = indexed_document::ActiveModel {
                                domain: Set(url_host.to_string()),
                                url: Set(url.as_str().to_string()),
                                doc_id: Set(new_id),
                                ..Default::default()
                            };

                            updates.push(update);
                        }
                    }
                }
            }

            let doc_insert = indexed_document::Entity::insert_many(updates)
                .on_conflict(
                    entities::sea_orm::sea_query::OnConflict::columns(vec![
                        indexed_document::Column::OpenUrl,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec(&transaction)
                .await;

            if let Err(error) = doc_insert {
                log::error!("Insert many failed {:?}", error);
            }

            let commit = transaction.commit().await;
            if let Err(error) = commit {
                log::error!("Failed to commit transaction {:?}", error);
            }
        }
        Err(err) => log::error!("Transaction failed {:?}", err),
    }
}
