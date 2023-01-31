use entities::models::crawl_queue::EnqueueSettings;
use shared::regex::{regex_for_robots, WildcardType};
use url::Url;

use entities::models::{
    bootstrap_queue, crawl_queue, crawl_tag, indexed_document,
    tag::{self, TagPair},
};
use entities::sea_orm::prelude::*;
use entities::sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};
use shared::config::{Config, LensConfig, LensRule, LensSource};
use std::collections::HashMap;

use super::{bootstrap, CollectTask, ManagerCommand};
use super::{CleanupTask, CrawlTask};
use crate::crawler::{CrawlError, CrawlResult, Crawler};
use crate::search::{DocumentUpdate, Searcher};
use crate::state::AppState;

/// Handles bootstrapping a lens. If the lens is remote we attempt to process the cache.
/// If no cache is accessible then we run the standard bootstrap process. Local lenses use
/// the standard bootstrap process
#[tracing::instrument(skip(state, config, lens))]
pub async fn handle_bootstrap_lens(state: &AppState, config: &Config, lens: &LensConfig) {
    log::debug!("Bootstrapping Lens {:?}", lens);
    match &lens.lens_source {
        LensSource::Remote(_) => {
            if !(bootstrap::bootstrap_lens_cache(state, config, lens).await) {
                process_lens(state, lens).await;
            }
        }
        _ => {
            process_lens(state, lens).await;
        }
    } // Deleted or no longer exists?
}

// Process lens by kicking off a bootstrap for the lens
async fn process_lens(state: &AppState, lens: &LensConfig) {
    for domain in lens.domains.iter() {
        let _ = state
            .schedule_work(ManagerCommand::Collect(CollectTask::CDXCollection {
                lens: lens.name.clone(),
                seed_url: format!("https://{domain}"),
                pipeline: lens.pipeline.as_ref().cloned(),
            }))
            .await;
    }

    process_urls(lens, state).await;
    process_lens_rules(lens, state).await;
}

// Process the urls by adding them to the crawl queue or bootstrapping the urls
async fn process_urls(lens: &LensConfig, state: &AppState) {
    let pipeline_kind = lens.pipeline.as_ref().cloned();

    for prefix in lens.urls.iter() {
        // Handle singular URL matches. Simply add these to the crawl queue.
        if prefix.ends_with('$') {
            // Remove the '$' suffix and add to the crawl queue
            let url = prefix.strip_suffix('$').expect("No $ at end of prefix");
            if let Err(err) = crawl_queue::enqueue_all(
                &state.db,
                &[url.to_owned()],
                &[],
                &state.user_settings,
                &EnqueueSettings {
                    force_allow: true,
                    ..Default::default()
                },
                pipeline_kind.clone(),
            )
            .await
            {
                log::warn!("unable to enqueue <{}> due to {}", prefix, err)
            }
        } else {
            // Otherwise, bootstrap using this as a prefix.
            let _ = state
                .schedule_work(ManagerCommand::Collect(CollectTask::CDXCollection {
                    lens: lens.name.clone(),
                    seed_url: prefix.to_string(),
                    pipeline: pipeline_kind.clone(),
                }))
                .await;
        }
    }
}

// Processes the len rules
async fn process_lens_rules(lens: &LensConfig, state: &AppState) {
    // Rules will go through and remove crawl tasks AND indexed_documents that match.
    for rule in lens.rules.iter() {
        match rule {
            LensRule::SkipURL(rule_str) => {
                if let Some(rule_like) = regex_for_robots(rule_str, WildcardType::Database) {
                    // Remove matching crawl tasks
                    let _ = crawl_queue::remove_by_rule(&state.db, &rule_like).await;
                    // Remove matching indexed documents
                    match indexed_document::remove_by_rule(&state.db, &rule_like).await {
                        Ok(doc_ids) => {
                            for doc_id in doc_ids {
                                let _ = Searcher::delete_by_id(state, &doc_id).await;
                            }
                            let _ = Searcher::save(state).await;
                        }
                        Err(e) => log::error!("Unable to remove docs: {:?}", e),
                    }
                }
            }
            LensRule::LimitURLDepth(rule_str, _) => {
                // Remove URLs that don't match this rule
                // sqlite3 does support regexp, but this is _not_ guaranteed to
                // be on all platforms, so we'll apply this in a brute-force way.
                if let Ok(parsed) = Url::parse(rule_str) {
                    if let Some(domain) = parsed.host_str() {
                        // Remove none matchin URLs from crawl_queue
                        let urls = crawl_queue::Entity::find()
                            .filter(crawl_queue::Column::Domain.eq(domain))
                            .all(&state.db)
                            .await;

                        let regex = regex::Regex::new(&rule.to_regex())
                            .expect("Invalid LimitURLDepth regex");

                        let mut num_removed = 0;
                        if let Ok(urls) = urls {
                            for crawl in urls {
                                if !regex.is_match(&crawl.url) {
                                    num_removed += 1;
                                    let _ = crawl.delete(&state.db).await;
                                }
                            }
                        }
                        log::info!("removed {} docs from crawl_queue", num_removed);

                        // Remove none matchin URLs from indexed documents
                        let mut num_removed = 0;
                        let indexed = indexed_document::Entity::find()
                            .filter(indexed_document::Column::Domain.eq(domain))
                            .all(&state.db)
                            .await;

                        let mut doc_ids = Vec::new();
                        if let Ok(indexed) = indexed {
                            for doc in indexed {
                                if !regex.is_match(&doc.url) {
                                    num_removed += 1;
                                    doc_ids.push(doc.doc_id.clone());
                                    let _ = doc.delete(&state.db).await;
                                }
                            }
                        }

                        for doc_id in doc_ids {
                            let _ = Searcher::delete_by_id(state, &doc_id).await;
                        }
                        let _ = Searcher::save(state).await;

                        log::info!("removed {} docs from indexed_documents", num_removed);
                    }
                }
            }
        }
    }
}

/// Helper used to cleanup the database when documents are in the index, but are missing from
/// the database. This typically happens when a cache has invalid content. Currently the
/// cleanup is minimal, but can be expanded in the future.
pub async fn cleanup_database(state: &AppState, cleanup_task: CleanupTask) -> anyhow::Result<()> {
    log::debug!(
        "Running database cleanup for the following {:?}",
        cleanup_task
    );
    let mut changed = false;
    if !&cleanup_task.missing_docs.is_empty() {
        for (doc_id, url) in cleanup_task.missing_docs {
            let doc = indexed_document::Entity::find()
                .filter(indexed_document::Column::Url.eq(url.clone()))
                .one(&state.db)
                .await;
            match doc {
                Ok(Some(doc_model)) => {
                    log::debug!("Found document for url {}", url);
                    if !doc_model.doc_id.eq(&doc_id) {
                        // Found document for the url, but it has a different doc id.
                        // check if this document exists in the index to see if we
                        // had a duplicate
                        let indexed_result =
                            Searcher::get_by_id(&state.index.reader, doc_model.doc_id.as_str());
                        match indexed_result {
                            Some(_doc) => {
                                log::debug!(
                                    "Found duplicate for url: {}, removing duplicate doc {}",
                                    url,
                                    doc_id
                                );
                                // Found indexed document, so we must have had duplicates, remove dup
                                let _ = Searcher::delete_by_id(state, doc_id.as_str()).await;
                                changed = true;
                            }
                            None => {
                                log::error!("No document found in index, db doc id is different than index doc id. Index: {} Database: {}",
                                    doc_id, doc_model.doc_id);
                                // best we could do is normalize the doc id. The issue is that if they have different doc ids, we do not
                                // know if they have the same / valid content.
                                let mut updated: indexed_document::ActiveModel =
                                    doc_model.clone().into();
                                updated.doc_id = Set(doc_id.clone());
                                let _ = updated.update(&state.db).await;
                            }
                        }
                    }
                }
                Ok(None) => {
                    log::debug!("Could not find document for url {}, removing", url);
                    // can't find the url at all must be an old doc that was removed
                    let _ = Searcher::delete_by_id(state, doc_id.as_str()).await;
                    changed = true;
                }
                Err(error) => {
                    // we had an error so can't say what happened
                    log::error!("Got error accessing url {}, error: {:?}", url, error);
                }
            }
        }
    }

    if changed {
        let _ = Searcher::save(state).await;
    }

    Ok(())
}

/// Check if we've already bootstrapped a prefix / otherwise add it to the queue.
#[tracing::instrument(skip(state, lens))]
pub async fn handle_bootstrap(
    state: &AppState,
    lens: &LensConfig,
    seed_url: &str,
    pipeline: Option<String>,
) -> bool {
    let db = &state.db;
    let user_settings = &state.user_settings;

    let url = Url::parse(seed_url);
    if url.is_err() {
        log::error!("{} is an invalid URL", seed_url);
        return false;
    }

    let url = url.expect("invalid url");
    if let Ok(false) = bootstrap_queue::has_seed_url(db, seed_url).await {
        match bootstrap::bootstrap(state, lens, db, user_settings, &url, pipeline).await {
            Err(e) => {
                log::error!("error bootstrapping <{}>: {}", url.to_string(), e);
                return false;
            }
            Ok(cnt) => {
                log::info!("bootstrapped {} w/ {} urls", seed_url, cnt);
                let _ = bootstrap_queue::enqueue(db, seed_url, cnt as i64).await;
                return true;
            }
        }
    } else {
        log::info!(
            "bootstrap queue already contains seed url: {}, skipping",
            seed_url
        );
    }

    false
}

#[derive(Debug, Eq, PartialEq)]
pub enum FetchResult {
    New,
    Error(CrawlError),
    Ignore,
    NotFound,
    Updated,
}

/// Handle multiple crawl results at once.
pub async fn add_document_and_tags(
    state: &AppState,
    result: &CrawlResult,
    extra_tags: &[TagPair],
) -> anyhow::Result<FetchResult> {
    // find all documents that already exist with that url
    let existing: Vec<indexed_document::Model> = indexed_document::Entity::find()
        .filter(indexed_document::Column::Url.eq(result.url.clone()))
        .all(&state.db)
        .await
        .unwrap_or_default();

    // build a hash map of Url to the doc id
    let id_map = existing
        .iter()
        .map(|model| (model.url.to_string(), model.doc_id.to_string()))
        .collect::<HashMap<String, String>>();

    // build a list of doc ids to delete from the index
    let doc_id_list = id_map
        .values()
        .into_iter()
        .map(|x| x.to_owned())
        .collect::<Vec<String>>();

    // Delete existing docs
    let _ = Searcher::delete_many_by_id(state, &doc_id_list, false).await;
    let _ = Searcher::save(state).await;

    let mut updates = Vec::new();
    let mut added_docs = Vec::new();

    // Find/create the tags for this crawl.
    let mut all_tags = Vec::new();
    all_tags.extend(&result.tags);
    all_tags.extend(extra_tags);
    let mut tags = Vec::new();
    for (label, value) in &all_tags {
        let tag = tag::get_or_create(&state.db, label.to_owned(), value).await?;
        tags.push(tag.id);
    }

    match Url::parse(&result.url) {
        Ok(url) => {
            let url_host = url.host_str().unwrap_or("");
            // Add document to index
            if let Ok(mut index_writer) = state.index.writer.lock() {
                let doc_id = Searcher::upsert_document(
                    &mut index_writer,
                    DocumentUpdate {
                        doc_id: id_map.get(&result.url).cloned(),
                        title: &result.title.clone().unwrap_or_default(),
                        description: &result.description.clone().unwrap_or_default(),
                        domain: url_host,
                        url: url.as_str(),
                        content: &result.content.clone().unwrap_or_default(),
                        tags: &Some(tags.clone()),
                    },
                )?;

                if !id_map.contains_key(&doc_id) {
                    added_docs.push(url.to_string());
                    let update = indexed_document::ActiveModel {
                        domain: Set(url_host.to_string()),
                        url: Set(url.to_string()),
                        open_url: Set(result.open_url.clone()),
                        doc_id: Set(doc_id),
                        ..Default::default()
                    };

                    updates.push(update);
                }
            }
        }
        Err(error) => log::error!("Error processing url: {} error: {:?}", result.url, error),
    }

    indexed_document::insert_many(&state.db, updates).await?;
    // Get ids of recently added docs
    let added = indexed_document::Entity::find()
        .filter(indexed_document::Column::Url.eq(result.url.clone()))
        .one(&state.db)
        .await?;

    if let Some(added) = added {
        let result = indexed_document::insert_tags_for_docs(&state.db, &[added], &tags).await;
        if let Err(error) = result {
            log::error!("Error inserting tags {:?}", error);
        }
    }

    if existing.is_empty() {
        Ok(FetchResult::New)
    } else {
        Ok(FetchResult::Updated)
    }
}

pub async fn process_crawl(
    state: &AppState,
    task_id: i64,
    crawl_result: &CrawlResult,
) -> anyhow::Result<FetchResult, CrawlError> {
    // Update job status
    let task =
        match crawl_queue::mark_done(&state.db, task_id, Some(crawl_result.tags.clone())).await {
            Some(task) => task,
            // Task removed while being processed?
            None => return Err(CrawlError::Other("task no longer exists".to_owned())),
        };

    // Update URL in crawl_task to match the canonical URL extracted in the crawl result.
    if task.url != crawl_result.url {
        log::debug!("Updating task URL {} -> {}", task.url, crawl_result.url);
        match crawl_queue::update_or_remove_task(&state.db, task.id, &crawl_result.url).await {
            Ok(updated) => {
                if updated.id != task.id {
                    log::debug!("Removed {}, duplicate canonical URL found", task.id);
                }
            }
            Err(err) => {
                log::error!("Unable to update task URL: {}", err);
            }
        };
    }

    let task_tags = task
        .find_related(tag::Entity)
        .all(&state.db)
        .await
        .unwrap_or_default()
        .iter()
        .map(|t| t.tag_pair())
        .collect::<Vec<TagPair>>();

    // Add all valid, non-duplicate, non-indexed links found to crawl queue
    let to_enqueue: Vec<String> = crawl_result.links.clone().into_iter().collect();

    // Grab enabled lenses
    let lenses: Vec<LensConfig> = state
        .lenses
        .iter()
        .filter(|entry| entry.value().pipeline.is_none())
        .map(|entry| entry.value().clone())
        .collect();

    if let Err(err) = crawl_queue::enqueue_all(
        &state.db,
        &to_enqueue,
        &lenses,
        &state.user_settings,
        &Default::default(),
        None,
    )
    .await
    {
        log::error!("error enqueuing all: {}", err);
    }

    // Add / update search index w/ crawl result.
    if crawl_result.content.is_none() {
        return Err(CrawlError::ParseError("No content found".to_string()));
    }

    match add_document_and_tags(state, crawl_result, &task_tags).await {
        Ok(res) => Ok(res),
        Err(err) => Err(CrawlError::Other(err.to_string())),
    }
}

#[tracing::instrument(skip(state))]
pub async fn handle_fetch(state: AppState, task: CrawlTask) -> FetchResult {
    let crawler = Crawler::new();
    let result = crawler.fetch_by_job(&state, task.id, true).await;

    match result {
        Ok(crawl_result) => match process_crawl(&state, task.id, &crawl_result).await {
            Ok(res) => {
                log::debug!("Crawled task id: {} - {:?}", task.id, res);
                res
            }
            Err(err) => {
                log::warn!("Unable to crawl id: {} - {:?}", task.id, err);
                FetchResult::Error(err)
            }
        },
        Err(err) => {
            log::warn!("Unable to crawl id: {} - {:?}", task.id, err);
            match err {
                // Ignore skips, recently fetched crawls, or not found
                CrawlError::Denied(_) | CrawlError::RecentlyFetched => {
                    let _ = crawl_queue::mark_done(&state.db, task.id, None).await;
                    FetchResult::Ignore
                }
                CrawlError::NotFound => {
                    let _ = crawl_queue::mark_done(&state.db, task.id, None).await;
                    FetchResult::NotFound
                }
                // Retry timeouts, might be a network issue
                CrawlError::Timeout => {
                    log::info!("Retrying task {} if possible", task.id);
                    crawl_queue::mark_failed(&state.db, task.id, true).await;
                    FetchResult::Error(err.clone())
                }
                // No need to retry these, mark as failed.
                CrawlError::FetchError(_)
                | CrawlError::ParseError(_)
                | CrawlError::Unsupported(_)
                | CrawlError::Other(_) => {
                    // mark crawl as failed
                    crawl_queue::mark_failed(&state.db, task.id, false).await;
                    FetchResult::Error(err.clone())
                }
            }
        }
    }
}

#[tracing::instrument(skip(state))]
pub async fn handle_deletion(state: AppState, task_id: i64) -> anyhow::Result<(), DbErr> {
    let task = crawl_queue::Entity::find_by_id(task_id)
        .one(&state.db)
        .await?;

    if let Some(task) = task {
        // Delete any associated tags
        crawl_tag::Entity::delete_many()
            .filter(crawl_tag::Column::CrawlQueueId.eq(task.id))
            .exec(&state.db)
            .await?;

        // Delete any documents that match this task.
        let docs = indexed_document::Entity::find()
            .filter(indexed_document::Column::Url.eq(task.url.clone()))
            .all(&state.db)
            .await?;

        // Grab doc ids to remove from index
        let doc_ids = docs
            .iter()
            .map(|x| x.doc_id.to_string())
            .collect::<Vec<String>>();

        // Remove doc references from DB & from index
        for doc_id in doc_ids {
            let _ = Searcher::delete_by_id(&state, &doc_id).await;
        }

        // Finally delete this crawl task as well.
        task.delete(&state.db).await?;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::crawler::CrawlResult;
    use crate::search::IndexPath;
    use entities::models::crawl_queue::{self, CrawlStatus, CrawlType};
    use entities::models::tag::{self, TagType};
    use entities::models::{bootstrap_queue, indexed_document};
    use entities::sea_orm::{ActiveModelTrait, EntityTrait, ModelTrait, Set};
    use entities::test::setup_test_db;
    use shared::config::UserSettings;

    use super::{handle_bootstrap, process_crawl, AppState, FetchResult};

    #[tokio::test]
    async fn test_handle_bootstrap() {
        let db = setup_test_db().await;
        let state = AppState::builder()
            .with_db(db)
            .with_user_settings(&UserSettings::default())
            .with_index(&IndexPath::Memory)
            .build();

        let test = "https://example.com";

        // Should skip this URL since we already have it.
        bootstrap_queue::enqueue(&state.db, test, 10).await.unwrap();
        assert!(!handle_bootstrap(&state, &Default::default(), test, None).await);
    }

    #[tokio::test]
    async fn test_process_crawl_new() {
        let db = setup_test_db().await;
        let state = AppState::builder()
            .with_db(db.clone())
            .with_user_settings(&UserSettings::default())
            .with_index(&IndexPath::Memory)
            .build();

        let model = crawl_queue::ActiveModel {
            domain: Set("example.com".to_owned()),
            url: Set("https://example.com/test".to_owned()),
            status: Set(CrawlStatus::Processing),
            crawl_type: Set(CrawlType::Normal),
            ..Default::default()
        };
        let task = model.insert(&db).await.expect("Unable to save model");

        let crawl_result = CrawlResult {
            content: Some("fake content".to_owned()),
            title: Some("Title".to_owned()),
            url: "https://example.com/test".to_owned(),
            ..Default::default()
        };

        // Should consider this a new FetchResult
        let result = process_crawl(&state, task.id, &crawl_result)
            .await
            .expect("success");
        assert_eq!(result, FetchResult::New);

        // Should update the task status
        let task = crawl_queue::Entity::find_by_id(task.id)
            .one(&db)
            .await
            .expect("Unable to query crawl task");
        assert_eq!(
            task.expect("Unable to find task").status,
            CrawlStatus::Completed
        );

        // Should add a new indexed_document obj
        let docs = indexed_document::Entity::find()
            .all(&db)
            .await
            .unwrap_or_default();
        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    async fn test_process_crawl_update() {
        let db = setup_test_db().await;
        let state = AppState::builder()
            .with_db(db.clone())
            .with_user_settings(&UserSettings::default())
            .with_index(&IndexPath::Memory)
            .build();

        let task = crawl_queue::ActiveModel {
            domain: Set("example.com".to_owned()),
            url: Set("https://example.com/test".to_owned()),
            status: Set(CrawlStatus::Processing),
            crawl_type: Set(CrawlType::Normal),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("Unable to save model");

        let _doc = indexed_document::ActiveModel {
            domain: Set(task.domain),
            url: Set(task.url),
            doc_id: Set("fake-doc-id".to_owned()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("Unable to save indexed_doc");

        let crawl_result = CrawlResult {
            content: Some("fake content".to_owned()),
            title: Some("Title".to_owned()),
            url: "https://example.com/test".to_owned(),
            ..Default::default()
        };

        let result = process_crawl(&state, task.id, &crawl_result)
            .await
            .expect("success");
        assert_eq!(result, FetchResult::Updated);

        // Should still only have one indexed doc
        // Should add a new indexed_document obj
        let docs = indexed_document::Entity::find()
            .all(&db)
            .await
            .unwrap_or_default();
        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    async fn test_process_crawl_new_with_tags() {
        let db = setup_test_db().await;
        let state = AppState::builder()
            .with_db(db.clone())
            .with_user_settings(&UserSettings::default())
            .with_index(&IndexPath::Memory)
            .build();

        let model = crawl_queue::ActiveModel {
            domain: Set("example.com".to_owned()),
            url: Set("https://example.com/test".to_owned()),
            status: Set(CrawlStatus::Processing),
            crawl_type: Set(CrawlType::Normal),
            ..Default::default()
        };
        let task = model.save(&db).await.expect("Unable to save model");
        let _ = task
            .insert_tags(&db, &[(TagType::Source, "web".to_string())])
            .await;

        let crawl_result = CrawlResult {
            content: Some("fake content".to_owned()),
            title: Some("Title".to_owned()),
            url: "https://example.com/test".to_owned(),
            ..Default::default()
        };

        // Should consider this a new FetchResult
        let task_id = task.id.unwrap();
        let result = process_crawl(&state, task_id, &crawl_result)
            .await
            .expect("success");
        assert_eq!(result, FetchResult::New);

        // Should update the task status
        let task = crawl_queue::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .expect("Unable to query crawl task");
        assert_eq!(
            task.expect("Unable to find task").status,
            CrawlStatus::Completed
        );

        // Should add a new indexed_document obj
        let docs = indexed_document::Entity::find()
            .all(&db)
            .await
            .unwrap_or_default();
        assert_eq!(docs.len(), 1);

        // Should have added the tag.
        let new_doc = docs.get(0).expect("new_doc");
        let tags = new_doc
            .find_related(tag::Entity)
            .all(&db)
            .await
            .unwrap_or_default();
        assert_eq!(tags.len(), 1);
        let tag = tags.get(0).expect("tags.get(0)");
        assert_eq!(tag.label, TagType::Source);
        assert_eq!(tag.value, "web".to_string());
    }

    #[tokio::test]
    async fn test_process_crawl_update_with_tags() {
        let db = setup_test_db().await;
        let state = AppState::builder()
            .with_db(db.clone())
            .with_user_settings(&UserSettings::default())
            .with_index(&IndexPath::Memory)
            .build();

        let task = crawl_queue::ActiveModel {
            domain: Set("example.com".to_owned()),
            url: Set("https://example.com/test".to_owned()),
            status: Set(CrawlStatus::Processing),
            crawl_type: Set(CrawlType::Normal),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("Unable to save model");
        let model: crawl_queue::ActiveModel = task.clone().into();
        let _ = model
            .insert_tags(
                &db,
                &[
                    (TagType::Source, "web".to_owned()),
                    (TagType::Lens, "lens".to_owned()),
                ],
            )
            .await;

        let doc = indexed_document::ActiveModel {
            domain: Set(task.domain),
            url: Set(task.url),
            doc_id: Set("fake-doc-id".to_owned()),
            ..Default::default()
        };
        let doc = doc.save(&db).await.expect("Unable to save indexed_doc");
        let _ = doc
            .insert_tags(&db, &[(TagType::Source, "web".to_owned())])
            .await;

        let crawl_result = CrawlResult {
            content: Some("fake content".to_owned()),
            title: Some("Title".to_owned()),
            url: "https://example.com/test".to_owned(),
            tags: vec![((TagType::MimeType, "application/pdf".to_owned()))],
            ..Default::default()
        };

        let result = process_crawl(&state, task.id, &crawl_result)
            .await
            .expect("success");
        assert_eq!(result, FetchResult::Updated);

        // Should still only have one indexed doc
        // Should add a new indexed_document obj
        let docs = indexed_document::Entity::find()
            .all(&db)
            .await
            .unwrap_or_default();
        assert_eq!(docs.len(), 1);

        // Should only add the new tag.
        let doc = docs.get(0).expect("docs.get");
        let tags = doc
            .find_related(tag::Entity)
            .all(&db)
            .await
            .unwrap_or_default();
        assert_eq!(tags.len(), 3);

        // CrawlResult should be updated w/ merged tags list
        let task = crawl_queue::Entity::find_by_id(task.id)
            .one(&db)
            .await
            .expect("find one");
        let task = task.expect("should have task");
        let task_tags = task
            .find_related(tag::Entity)
            .all(&db)
            .await
            .unwrap_or_default();
        assert_eq!(task_tags.len(), 3);
    }
}
