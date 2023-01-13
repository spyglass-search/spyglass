use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::crawler::{cache, CrawlResult};
use crate::pipeline::collector::CollectionResult;
use crate::pipeline::PipelineContext;
use crate::search::Searcher;
use crate::state::AppState;

use entities::models::tag::TagType;
use entities::models::{indexed_document, tag};
use libnetrunner::parser::ParseResult;
use tokio::task::JoinHandle;

use super::parser::DefaultParser;
use crate::crawler::archive;
use entities::sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use url::Url;

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
pub async fn process_update(state: AppState, lens: String, cache_path: PathBuf) {
    let now = Instant::now();
    let records = archive::read_parsed(&cache_path);
    if let Ok(mut record_iter) = records {
        let mut record_list: Vec<ParseResult> = Vec::new();
        for record in record_iter.by_ref() {
            record_list.push(record);
            if record_list.len() >= 5000 {
                process_records(&state, &lens, &mut record_list).await;
                record_list = Vec::new();
            }
        }

        if !record_list.is_empty() {
            process_records(&state, &lens, &mut record_list).await;
        }
    }

    // attempt to remove processed cache file
    let _ = cache::delete_cache(&cache_path);
    log::debug!("Processing Cache Took: {:?}", now.elapsed().as_millis())
}

// Process a list of crawl results. The following steps will be taken:
// 1. Find all urls that already have been processed in the database
// 2. Remove any documents that already exist from the index
// 3. Add all new results to the index
// 4. Insert all new documents to the indexed document database
async fn process_records(state: &AppState, lens: &str, results: &mut Vec<ParseResult>) {
    // get a list of all urls
    let parsed_urls = results
        .iter()
        .map(|val| val.canonical_url.clone().unwrap_or_default())
        .collect::<Vec<String>>();

    // find all documents that already exist with that url
    let existing: Vec<indexed_document::Model> = indexed_document::Entity::find()
        .filter(indexed_document::Column::Url.is_in(parsed_urls))
        .all(&state.db)
        .await
        .unwrap_or_default();

    // build a hash map of Url to the doc id
    let mut id_map = HashMap::new();
    for model in &existing {
        let _ = id_map.insert(model.url.to_string(), model.doc_id.clone());
    }

    // build a list of doc ids to delete from the index
    let doc_id_list = id_map.values().map(AsRef::as_ref).collect::<Vec<&str>>();

    let _ = Searcher::delete_many_by_id(state, &doc_id_list, false).await;
    let _ = Searcher::save(state).await;

    // Access tag for this lens and build id list
    let tag = tag::get_or_create(&state.db, TagType::Lens, lens).await;
    let tag_list = match tag {
        Ok(model) => Some(vec![model.id as u64]),
        Err(error) => {
            log::error!("Error accessing tag for lens {:?}", error);
            None
        }
    };

    let transaction_rslt = state.db.begin().await;
    match transaction_rslt {
        Ok(transaction) => {
            let mut updates = Vec::new();
            let mut added_docs = Vec::new();
            for crawl_result in results {
                let canonical_url = crawl_result.canonical_url.clone();
                match canonical_url {
                    Some(canonical_url_str) => {
                        let url_rslt = Url::parse(canonical_url_str.as_str());
                        match url_rslt {
                            Ok(url) => {
                                let url_host = url.host_str().unwrap_or("");
                                // Add document to index
                                let doc_id: Option<String> = {
                                    if let Ok(mut index_writer) = state.index.writer.lock() {
                                        match Searcher::upsert_document(
                                            &mut index_writer,
                                            id_map.get(&canonical_url_str).cloned(),
                                            &crawl_result.title.clone().unwrap_or_default(),
                                            &crawl_result.description.clone(),
                                            url_host,
                                            url.as_str(),
                                            &crawl_result.content,
                                            &tag_list,
                                        ) {
                                            Ok(new_doc_id) => Some(new_doc_id),
                                            _ => None,
                                        }
                                    } else {
                                        None
                                    }
                                };

                                if let Some(new_id) = doc_id {
                                    if !id_map.contains_key(&new_id) {
                                        added_docs.push(url.to_string());
                                        let update = indexed_document::ActiveModel {
                                            domain: Set(url_host.to_string()),
                                            url: Set(url.to_string()),
                                            open_url: Set(Some(url.to_string())),
                                            doc_id: Set(new_id),
                                            ..Default::default()
                                        };

                                        updates.push(update);
                                    }
                                }
                            }
                            Err(error) => log::error!(
                                "Error processing url: {:?} error: {:?}",
                                canonical_url_str,
                                error
                            ),
                        }
                    }
                    None => log::error!(
                        "None url is not value for content {:?}",
                        crawl_result.content.truncate(80)
                    ),
                }
            }

            let doc_insert = indexed_document::Entity::insert_many(updates)
                .on_conflict(
                    entities::sea_orm::sea_query::OnConflict::columns(vec![
                        indexed_document::Column::Url,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec(&transaction)
                .await;

            if let Err(error) = doc_insert {
                log::error!("Docs failed to insert {:?}", error);
            }

            let commit = transaction.commit().await;
            match commit {
                Ok(_) => {
                    let added_entries: Vec<indexed_document::Model> =
                        indexed_document::Entity::find()
                            .filter(indexed_document::Column::Url.is_in(added_docs))
                            .all(&state.db)
                            .await
                            .unwrap_or_default();

                    if !added_entries.is_empty() {
                        let result = indexed_document::insert_tags_many(
                            &added_entries,
                            &state.db,
                            &[(TagType::Lens, lens.to_string())],
                        )
                        .await;
                        if let Err(error) = result {
                            log::error!("Error inserting tags {:?}", error);
                        }
                    }
                }
                Err(error) => {
                    log::error!("Failed to commit transaction {:?}", error);
                }
            }
        }
        Err(err) => log::error!("Transaction failed {:?}", err),
    }
}
