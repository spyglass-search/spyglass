use entities::{
    models::{
        crawl_queue, indexed_document,
        tag::{self, TagPair},
    },
    sea_orm::DatabaseConnection,
};
use std::{collections::HashMap, time::Instant};

use libnetrunner::parser::ParseResult;
use url::Url;

use crate::{
    crawler::CrawlResult,
    search::{DocumentUpdate, Searcher},
    state::AppState,
};
use entities::models::tag::TagType;
use entities::sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};

/// Helper method to delete indexed documents, crawl queue items and search
/// documents by url
pub async fn delete_documents_by_uri(state: &AppState, uri: Vec<String>) {
    log::debug!("Deleting {:?} documents", uri.len());

    // Delete from crawl queue

    if let Err(error) = crawl_queue::delete_many_by_url(&state.db, &uri).await {
        log::error!("Error delete items from crawl queue {:?}", error);
    }

    // find all documents that already exist with that url
    let existing: Vec<indexed_document::Model> = indexed_document::Entity::find()
        .filter(indexed_document::Column::Url.is_in(uri.clone()))
        .all(&state.db)
        .await
        .unwrap_or_default();

    // build a hash map of Url to the doc id
    let mut id_map = HashMap::new();
    for model in &existing {
        let _ = id_map.insert(model.url.to_string(), model.doc_id.clone());
    }

    // build a list of doc ids to delete from the index
    let doc_id_list = id_map
        .values()
        .into_iter()
        .map(|x| x.to_owned())
        .collect::<Vec<String>>();

    let _ = Searcher::delete_many_by_id(state, &doc_id_list, false).await;
    let _ = Searcher::save(state).await;

    // now that the documents are deleted delete from the queue
    if let Err(error) = indexed_document::delete_many_by_url(&state.db, uri).await {
        log::error!("Error deleting for indexed document store {:?}", error);
    }
}

pub struct AddUpdateResult {
    pub num_added: usize,
    pub num_updated: usize,
}

/// Process a list of crawl results. The following steps will be taken:
/// 1. Find all urls that already have been processed in the database
/// 2. Remove any documents that already exist from the index
/// 3. Add all new results to the index
/// 4. Insert all new documents to the indexed document database
pub async fn process_crawl_results(
    state: &AppState,
    results: &[CrawlResult],
    global_tags: &[TagPair],
) -> anyhow::Result<AddUpdateResult> {
    let now = Instant::now();
    // get a list of all urls
    let parsed_urls = results
        .iter()
        .map(|val| val.url.clone())
        .collect::<Vec<String>>();

    // find all documents that already exist with that url
    let existing: Vec<indexed_document::Model> = indexed_document::Entity::find()
        .filter(indexed_document::Column::Url.is_in(parsed_urls))
        .all(&state.db)
        .await
        .unwrap_or_default();

    // build a hash map of Url to the doc id
    let id_map = existing
        .iter()
        .map(|model| (model.url.to_string(), model.doc_id.to_string()))
        .collect::<HashMap<String, String>>();

    // build a list of doc ids to delete from the index
    let doc_id_list = id_map.values().cloned().collect::<Vec<String>>();

    // Delete existing docs
    let _ = Searcher::delete_many_by_id(state, &doc_id_list, false).await;
    let _ = Searcher::save(state).await;

    // Find/create the tags for this crawl.
    let mut tag_map: HashMap<String, Vec<i64>> = HashMap::new();
    let mut tag_cache = HashMap::new();

    // Grab tags that applies to all crawl results.
    let global_tids = _get_tag_ids(&state.db, global_tags, &mut tag_cache).await;

    // Keep track of document upserts
    let mut updates = Vec::new();
    let mut added_docs = Vec::new();

    let tx = state.db.begin().await?;
    for crawl_result in results {
        // Fetch the tag ids to apply to this crawl.
        let mut tags_for_crawl = _get_tag_ids(&state.db, &crawl_result.tags, &mut tag_cache).await;
        tags_for_crawl.extend(global_tids.clone());
        tag_map.insert(crawl_result.url.clone(), tags_for_crawl.clone());

        // Add document to index
        let url = Url::parse(&crawl_result.url)?;
        let url_host = url.host_str().unwrap_or("");
        // Add document to index
        if let Ok(mut index_writer) = state.index.writer.lock() {
            let doc_id = Searcher::upsert_document(
                &mut index_writer,
                DocumentUpdate {
                    doc_id: id_map.get(&crawl_result.url).cloned(),
                    title: &crawl_result.title.clone().unwrap_or_default(),
                    description: &crawl_result.description.clone().unwrap_or_default(),
                    domain: url_host,
                    url: url.as_str(),
                    content: &crawl_result.content.clone().unwrap_or_default(),
                    tags: &Some(tags_for_crawl.clone()),
                },
            )?;

            if !id_map.contains_key(&doc_id) {
                added_docs.push(url.to_string());
                updates.push(indexed_document::ActiveModel {
                    domain: Set(url_host.to_string()),
                    url: Set(url.to_string()),
                    open_url: Set(crawl_result.open_url.clone()),
                    doc_id: Set(doc_id),
                    ..Default::default()
                });
            }
        }
    }

    // Insert docs & save everything.
    indexed_document::insert_many(&tx, updates).await?;
    tx.commit().await?;
    let _ = Searcher::save(state).await;

    // Find the recently added docs & apply the tags to them.
    let added_entries: Vec<indexed_document::Model> = indexed_document::Entity::find()
        .filter(indexed_document::Column::Url.is_in(added_docs))
        .all(&state.db)
        .await
        .unwrap_or_default();

    let tx = state.db.begin().await?;
    let num_entries = added_entries.len();
    for added in added_entries {
        if let Some(tag_ids) = tag_map.get(&added.url) {
            if let Err(err) = indexed_document::insert_tags_for_docs(&tx, &[added], tag_ids).await {
                log::error!("Error inserting tags {:?}", err);
            }
        }
    }
    tx.commit().await?;

    log::debug!(
        "Took {:?} to process crawl results",
        now.elapsed().as_millis()
    );

    let num_updates = existing.len();
    Ok(AddUpdateResult {
        num_added: num_entries - num_updates,
        num_updated: num_updates,
    })
}

// Process a list of crawl results. The following steps will be taken:
// 1. Find all urls that already have been processed in the database
// 2. Remove any documents that already exist from the index
// 3. Add all new results to the index
// 4. Insert all new documents to the indexed document database
pub async fn process_records(state: &AppState, lens: &str, results: &mut Vec<ParseResult>) {
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
    let doc_id_list = id_map
        .values()
        .into_iter()
        .map(|x| x.to_owned())
        .collect::<Vec<String>>();

    let _ = Searcher::delete_many_by_id(state, &doc_id_list, false).await;
    let _ = Searcher::save(state).await;

    // Access tag for this lens and build id list
    let tag = tag::get_or_create(&state.db, TagType::Lens, lens).await;
    let tag_list = match tag {
        Ok(model) => Some(vec![model.id]),
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
                                            DocumentUpdate {
                                                doc_id: id_map.get(&canonical_url_str).cloned(),
                                                title: &crawl_result
                                                    .title
                                                    .clone()
                                                    .unwrap_or_default(),
                                                description: &crawl_result.description.clone(),
                                                domain: url_host,
                                                url: url.as_str(),
                                                content: &crawl_result.content,
                                                tags: &tag_list,
                                            },
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
                    if let Ok(mut writer) = state.index.writer.lock() {
                        let _ = writer.commit();
                    }

                    let added_entries: Vec<indexed_document::Model> =
                        indexed_document::Entity::find()
                            .filter(indexed_document::Column::Url.is_in(added_docs))
                            .all(&state.db)
                            .await
                            .unwrap_or_default();

                    if !added_entries.is_empty() {
                        let result = indexed_document::insert_tags_many(
                            &state.db,
                            &added_entries,
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

/// Helper method used to get the tag ids for a specific crawl result. The tag map and the tag cache
/// will be modified as results are processed. The tag map contains the url to tag it mapping used
/// for insertion to the database. The tag_cache is used to avoid additional loops for common tags
/// that have already been processed.
async fn _get_tag_ids(
    db: &DatabaseConnection,
    tags: &[TagPair],
    tag_cache: &mut HashMap<String, i64>,
) -> Vec<i64> {
    let mut tids = Vec::new();
    let mut to_search = Vec::new();

    for (tag_type, value) in tags {
        let uid = format!("{tag_type}:{value}");
        if let Some(id) = tag_cache.get(&uid) {
            tids.push(*id);
        } else {
            to_search.push((tag_type.clone(), value.clone()));
        }
    }

    if !to_search.is_empty() {
        match tag::get_or_create_many(db, &to_search).await {
            Ok(tag_models) => {
                for tag in tag_models {
                    let tag_id = tag.id;
                    tids.push(tag_id);
                    tag_cache.insert(format!("{}:{}", tag.label, tag.value), tag_id);
                }
            }
            Err(error) => {
                log::error!("Error accessing or creating tags {:?}", error);
            }
        }
    }

    tids
}
