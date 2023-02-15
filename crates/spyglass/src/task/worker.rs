use entities::models::crawl_queue::EnqueueSettings;

use entities::models::{
    bootstrap_queue, crawl_queue, crawl_tag, indexed_document,
    tag::{self, TagPair},
};
use entities::sea_orm::prelude::*;
use entities::sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};
use shared::config::{Config, LensConfig, LensSource};

use super::{bootstrap, CollectTask, ManagerCommand};
use super::{CleanupTask, CrawlTask};
use crate::search::Searcher;
use crate::state::AppState;
use crate::{
    crawler::{CrawlError, CrawlResult, Crawler},
    documents::process_crawl_results,
};

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
    let _ = state
        .schedule_work(ManagerCommand::Collect(CollectTask::CDXCollection {
            lens: lens.name.clone(),
            pipeline: lens.pipeline.as_ref().cloned(),
        }))
        .await;
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
/// - Returns true if we've successfully run bootstrap
/// - Returns false if bootstrapping has been run already
/// - Returns an Error if we're unable to bootstrap for some reason.
#[tracing::instrument(skip(state, lens))]
pub async fn handle_cdx_collection(
    state: &AppState,
    lens: &LensConfig,
    pipeline: Option<String>,
) -> anyhow::Result<bool> {
    if bootstrap_queue::is_bootstrapped(&state.db, &lens.name).await? {
        return Ok(false);
    }

    let cnt = bootstrap::bootstrap(state, lens, pipeline).await?;
    log::info!("bootstrapped {} w/ {} urls", lens.name, cnt);
    let _ = bootstrap_queue::enqueue(&state.db, &lens.name, cnt as i64).await;
    Ok(true)
}

#[derive(Debug, Eq, PartialEq)]
pub enum FetchResult {
    New,
    Error(CrawlError),
    Ignore,
    NotFound,
    Updated,
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
        &EnqueueSettings {
            tags: task_tags.clone(),
            ..Default::default()
        },
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

    match process_crawl_results(state, &[crawl_result.clone()], &task_tags).await {
        Ok(res) => {
            if res.num_updated > 0 {
                Ok(FetchResult::Updated)
            } else {
                Ok(FetchResult::New)
            }
        }
        Err(err) => Err(CrawlError::Other(err.to_string())),
    }
}

#[tracing::instrument(skip(state))]
pub async fn handle_fetch(state: AppState, task: CrawlTask) -> FetchResult {
    let crawler = Crawler::new(state.user_settings.domain_crawl_limit.value());
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
    use shared::config::{LensConfig, UserSettings};

    use super::{handle_cdx_collection, process_crawl, AppState, FetchResult};

    #[tokio::test]
    async fn test_handle_cdx_collection() {
        let mut lens = LensConfig::default();
        lens.name = "example_lens".to_string();

        let db = setup_test_db().await;
        let state = AppState::builder()
            .with_db(db)
            .with_user_settings(&UserSettings::default())
            .with_index(&IndexPath::Memory)
            .build();

        // Should skip this lens since it's been bootstrapped already.
        bootstrap_queue::enqueue(&state.db, &lens.name, 10)
            .await
            .expect("Unable to add to bootstrap_queue");
        assert!(!handle_cdx_collection(&state, &lens, None)
            .await
            .expect("unable to run"));
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
