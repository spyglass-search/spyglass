use chrono::Utc;
use entities::{
    models::{
        crawl_queue,
        indexed_document::{self, find_by_doc_ids},
        tag::{self, TagPair},
    },
    sea_orm::{ActiveModelTrait, DatabaseConnection},
    BATCH_SIZE,
};
use shared::config::LensConfig;
use spyglass_plugin::TagModification;
use std::{collections::HashMap, str::FromStr, time::Instant};
use tantivy::DocAddress;

use libnetrunner::parser::ParseResult;
use url::Url;

use crate::{
    crawler::CrawlResult,
    search::{self, DocumentUpdate, RetrievedDocument, Searcher},
    state::AppState,
};
use entities::models::tag::TagType;
use entities::sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};

/// Helper method to delete indexed documents, crawl queue items and search
/// documents by url
pub async fn delete_documents_by_uri(state: &AppState, uri: Vec<String>) {
    log::info!("Deleting {} documents", uri.len());

    // Delete from crawl queue
    if let Err(error) = crawl_queue::delete_many_by_url(&state.db, &uri).await {
        log::error!("Error delete items from crawl queue {:?}", error);
    }

    // find all documents that already exist with that url
    for chunk in uri.chunks(BATCH_SIZE) {
        let existing: Vec<indexed_document::Model> = indexed_document::Entity::find()
            .filter(indexed_document::Column::Url.is_in(chunk.to_vec()))
            .all(&state.db)
            .await
            .unwrap_or_default();

        // build a hash map of Url to the doc id
        let mut id_map = HashMap::new();
        for model in &existing {
            id_map.insert(model.url.to_string(), model.doc_id.clone());
        }

        // build a list of doc ids to delete from the index
        let doc_id_list = id_map
            .values()
            .map(|x| x.to_owned())
            .collect::<Vec<String>>();

        if let Err(err) = Searcher::delete_many_by_id(state, &doc_id_list, false).await {
            log::warn!("Unable to delete_many_by_id: {err}")
        }

        if let Err(err) = Searcher::save(state).await {
            log::warn!("Unable to save searcher: {err}")
        }

        // now that the documents are deleted delete from the queue
        if let Err(error) = indexed_document::delete_many_by_url(&state.db, chunk).await {
            log::warn!("Error deleting for indexed document store {:?}", error);
        }

        log::info!("chunk: deleted {} ({}) docs from index", chunk.len(), existing.len());
    }
}

#[derive(Default)]
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
    if results.is_empty() {
        return Ok(AddUpdateResult::default());
    }

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
    let mut id_map = HashMap::new();
    let mut model_map = HashMap::new();
    for model in &existing {
        id_map.insert(model.url.to_string(), model.doc_id.to_string());
        model_map.insert(model.doc_id.to_string(), model.clone());
    }

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
    let mut inserts = Vec::new();
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
                    tags: &tags_for_crawl.clone(),
                },
            )?;

            if !id_map.contains_key(&doc_id) {
                added_docs.push(url.to_string());
                inserts.push(indexed_document::ActiveModel {
                    domain: Set(url_host.to_string()),
                    url: Set(url.to_string()),
                    open_url: Set(crawl_result.open_url.clone()),
                    doc_id: Set(doc_id),
                    updated_at: Set(Utc::now()),
                    ..Default::default()
                });
            } else if let Some(model) = model_map.get(&doc_id) {
                // Touch the existing model so we know it's been checked recently.
                let mut update: indexed_document::ActiveModel = model.to_owned().into();
                update.updated_at = Set(Utc::now());
                updates.push(update);
            }
        }
    }

    // Insert docs & save everything.
    indexed_document::insert_many(&tx, &inserts).await?;
    for update in updates {
        let _ = update.save(&tx).await;
    }

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
        "Took {}ms to process crawl {num_entries} results",
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
pub async fn process_records(
    state: &AppState,
    lens: &LensConfig,
    results: &mut Vec<ParseResult>,
) -> anyhow::Result<()> {
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
        .map(|x| x.to_owned())
        .collect::<Vec<String>>();

    let _ = Searcher::delete_many_by_id(state, &doc_id_list, false).await;
    let _ = Searcher::save(state).await;

    // Grab tags from the lens.
    let tags = lens
        .all_tags()
        .iter()
        .flat_map(|(label, value)| {
            if let Ok(tag_type) = TagType::from_str(label.as_str()) {
                Some((tag_type, value.clone()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // Create/grab tags from db.
    let tag_list = tag::get_or_create_many(&state.db, &tags)
        .await
        .unwrap_or_default()
        .iter()
        .map(|x| x.id)
        .collect::<Vec<_>>();

    let transaction = state.db.begin().await?;
    let mut updates = Vec::new();
    let mut added_docs = Vec::new();
    for crawl_result in results {
        if let Some(canonical_url_str) = &crawl_result.canonical_url {
            match Url::parse(canonical_url_str) {
                Ok(url) => {
                    let url_host = url.host_str().unwrap_or("");
                    // Add document to index
                    let doc_id: Option<String> = {
                        if let Ok(mut index_writer) = state.index.writer.lock() {
                            match Searcher::upsert_document(
                                &mut index_writer,
                                DocumentUpdate {
                                    doc_id: id_map.get(&canonical_url_str.clone()).cloned(),
                                    title: &crawl_result.title.clone().unwrap_or_default(),
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
        } else {
            log::warn!("Invalid canonical URL: {:?}", crawl_result.title.clone())
        }
    }

    // Save the data
    indexed_document::insert_many(&transaction, &updates).await?;
    transaction.commit().await?;
    if let Ok(mut writer) = state.index.writer.lock() {
        let _ = writer.commit();
    }

    let added_entries: Vec<indexed_document::Model> = indexed_document::Entity::find()
        .filter(indexed_document::Column::Url.is_in(added_docs))
        .all(&state.db)
        .await
        .unwrap_or_default();

    if !added_entries.is_empty() {
        if let Err(err) =
            indexed_document::insert_tags_for_docs(&state.db, &added_entries, &tag_list).await
        {
            log::error!("Error inserting tags {err}");
        }
    }

    Ok(())
}

/// Processes an update tags request for the specified documents
/// 1. Adds any tags to the database that are not present (database)
/// 2. Accesses documents from the index (index)
/// 3. Accesses documents and adds or removes associated tags (database)
/// 4. Gets all tag ids associated with a document (database)
/// 5. Updates the indexed document with the new tags (index)
pub async fn update_tags(
    state: &AppState,
    doc_ids: &[DocAddress],
    tag_modifications: &TagModification,
) -> anyhow::Result<()> {
    let mut tag_cache: HashMap<String, i64> = HashMap::new();
    let add_ids = match &tag_modifications.add {
        Some(to_add) => _get_tag_ids_string(&state.db, to_add, &mut tag_cache).await,
        None => Vec::new(),
    };

    let remove_ids = match &tag_modifications.remove {
        Some(to_add) => _get_tag_ids_string(&state.db, to_add, &mut tag_cache).await,
        None => Vec::new(),
    };

    let documents = doc_ids
        .iter()
        .filter_map(|addr| {
            if let Ok(doc) = state.index.reader.searcher().doc(*addr) {
                if let Ok(retrieved_doc) = search::document_to_struct(&doc) {
                    return Some(retrieved_doc);
                }
            }
            None
        })
        .collect::<Vec<RetrievedDocument>>();

    let document_ids = &documents
        .iter()
        .map(|doc| doc.doc_id.clone())
        .collect::<Vec<String>>();

    let doc_uuids = find_by_doc_ids(&state.db, document_ids)
        .await
        .unwrap_or_default()
        .iter()
        .map(|id| id.id)
        .collect::<Vec<i64>>();

    let mut updated = false;
    if !add_ids.is_empty() {
        log::debug!(
            "Inserting {} new tags and attaching to documents",
            add_ids.len()
        );
        if let Err(err) =
            indexed_document::insert_tags_for_docs_by_id(&state.db, &doc_uuids, &add_ids, false)
                .await
        {
            log::error!("Error inserting tags {:?}", err);
            return Err(anyhow::format_err!(err));
        }
        updated = true;
    }

    if !remove_ids.is_empty() {
        log::debug!("Removing tags {} from documents", remove_ids.len());
        if let Err(err) =
            indexed_document::remove_tags_for_docs_by_id(&state.db, &doc_uuids, &add_ids).await
        {
            log::error!("Error removing tags {:?}", err);
            return Err(anyhow::format_err!(err));
        }
        updated = true;
    }

    log::debug!("An update was made? {}", updated);
    if updated {
        let mut tag_map: HashMap<String, (RetrievedDocument, Vec<i64>)> = HashMap::new();
        for doc in documents {
            match indexed_document::get_tag_ids_by_doc_id(&state.db, &doc.doc_id).await {
                Ok(ids) => {
                    tag_map.insert(
                        doc.doc_id.clone(),
                        (
                            doc,
                            ids.iter().map(|tag_id| tag_id.id).collect::<Vec<i64>>(),
                        ),
                    );
                }
                Err(error) => {
                    log::error!(
                        "Unable to update document, could not access tags {:?}",
                        error
                    );
                }
            }
        }

        let _ = Searcher::delete_many_by_id(state, document_ids, false).await;
        let _ = Searcher::save(state).await;

        log::debug!("Tag map generated {}", tag_map.len());
        if let Ok(mut index_writer) = state.index.writer.lock() {
            for (_, (doc, ids)) in tag_map.iter() {
                let _doc_id = Searcher::upsert_document(
                    &mut index_writer,
                    DocumentUpdate {
                        doc_id: Some(doc.doc_id.clone()),
                        title: &doc.title,
                        description: &doc.description,
                        domain: &doc.domain,
                        url: &doc.url,
                        content: &doc.content,
                        tags: ids,
                    },
                )?;
            }
        }
    }

    Ok(())
}

/// Helper method used to get the tag ids for a specific crawl result. The tag map and the tag cache
/// will be modified as results are processed. The tag map contains the url to tag it mapping used
/// for insertion to the database. The tag_cache is used to avoid additional loops for common tags
/// that have already been processed.
async fn _get_tag_ids_string(
    db: &DatabaseConnection,
    tags: &[(String, String)],
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
        match tag::get_or_create_many_string(db, &to_search).await {
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
        let uid = format!("{tag_type:?}:{value}");
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
