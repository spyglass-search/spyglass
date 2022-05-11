use entities::sea_orm::prelude::*;
use entities::sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};
use tokio::sync::{broadcast, mpsc};
use url::Url;

use crate::crawler::Crawler;
use crate::search::Searcher;
use crate::state::AppState;
use entities::models::{crawl_queue, indexed_document};

#[derive(Debug, Clone)]
pub struct CrawlTask {
    pub id: i64,
}

#[derive(Debug)]
pub enum Command {
    Fetch(CrawlTask),
}

#[derive(Clone, Debug)]
pub enum AppShutdown {
    Now,
}

/// Manages the crawl queue
#[tracing::instrument(skip_all)]
pub async fn manager_task(
    state: AppState,
    queue: mpsc::Sender<Command>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    log::info!("manager started");

    loop {
        let mut prioritized_domains: Vec<String> = Vec::new();
        let mut prioritized_prefixes: Vec<String> = Vec::new();

        for entry in state.lenses.iter() {
            let value = entry.value();
            prioritized_domains.extend(value.domains.clone());
            prioritized_prefixes.extend(value.urls.clone());
        }

        // tokio::select allows us to listen to a shutdown message while
        // also processing queue tasks.
        let next_url = tokio::select! {
            res = crawl_queue::dequeue(
                &state.db,
                state.user_settings.clone(),
                &prioritized_domains,
                &prioritized_prefixes,
            ) => res.unwrap(),
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down manager");
                return;
            }
        };

        if let Some(task) = next_url {
            // Mark in progress
            let task_id = task.id;
            let mut update: crawl_queue::ActiveModel = task.into();
            update.status = Set(crawl_queue::CrawlStatus::Processing);
            update.update(&state.db).await.unwrap();

            // Send to worker
            let cmd = Command::Fetch(CrawlTask { id: task_id });
            if queue.send(cmd).await.is_err() {
                eprintln!("unable to send command to worker");
                return;
            }
        }
    }
}

#[tracing::instrument(skip(state, crawler))]
async fn _handle_fetch(state: AppState, crawler: Crawler, task: CrawlTask) {
    let result = crawler.fetch_by_job(&state.db, task.id).await;

    match result {
        Ok(Some(crawl_result)) => {
            // Update job status
            // We consider 400s complete in this case since we manage to hit the server
            // successfully but nothing useful was returned.
            let cq_status = if crawl_result.is_success() || crawl_result.is_bad_request() {
                crawl_queue::CrawlStatus::Completed
            } else {
                crawl_queue::CrawlStatus::Failed
            };

            crawl_queue::mark_done(&state.db, task.id, cq_status)
                .await
                .unwrap();

            // Add all valid, non-duplicate, non-indexed links found to crawl queue
            let to_enqueue: Vec<String> = crawl_result.links.into_iter().collect();
            if let Err(err) = crawl_queue::enqueue_all(
                &state.db,
                &to_enqueue,
                &state.user_settings,
                &Default::default(),
            )
            .await
            {
                log::error!("error enqueuing all: {}", err);
            }

            // Only add valid urls
            // if added.is_none() || added.unwrap() == crawl_queue::SkipReason::Duplicate {
            //     link::save_link(&state.db, &crawl_result.url, link)
            //         .await
            //         .unwrap();
            // }

            // Add / update search index w/ crawl result.
            if let Some(content) = crawl_result.content {
                let url = Url::parse(&crawl_result.url).unwrap();

                let existing = indexed_document::Entity::find()
                    .filter(indexed_document::Column::Url.eq(url.as_str()))
                    .one(&state.db)
                    .await
                    .unwrap();

                // Delete old document, if any.
                if let Some(doc) = &existing {
                    let mut index_writer = state.index.writer.lock().unwrap();
                    Searcher::delete(&mut index_writer, &doc.doc_id).unwrap();
                }

                // Add document to index
                let doc_id = {
                    let mut index_writer = state.index.writer.lock().unwrap();
                    Searcher::add_document(
                        &mut index_writer,
                        &crawl_result.title.unwrap_or_default(),
                        &crawl_result.description.unwrap_or_default(),
                        url.host_str().unwrap(),
                        url.as_str(),
                        &content,
                        &crawl_result.raw.unwrap(),
                    )
                    .unwrap()
                };

                // Update/create index reference in our database
                let indexed = if let Some(doc) = existing {
                    let mut update: indexed_document::ActiveModel = doc.into();
                    update.doc_id = Set(doc_id);
                    update
                } else {
                    indexed_document::ActiveModel {
                        domain: Set(url.host_str().unwrap().to_string()),
                        url: Set(url.as_str().to_string()),
                        doc_id: Set(doc_id),
                        ..Default::default()
                    }
                };

                indexed.save(&state.db).await.unwrap();
            }
        }
        Ok(None) => {
            // Failed to grab robots.txt or crawling is not allowed
            crawl_queue::mark_done(&state.db, task.id, crawl_queue::CrawlStatus::Completed)
                .await
                .unwrap();
        }
        Err(err) => {
            // mark crawl as failed
            crawl_queue::mark_done(&state.db, task.id, crawl_queue::CrawlStatus::Failed)
                .await
                .unwrap();
            log::error!("Unable to crawl id: {} - {:?}", task.id, err)
        }
    }
}

/// Grabs a task
pub async fn worker_task(
    state: AppState,
    mut queue: mpsc::Receiver<Command>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    log::info!("worker started");
    let crawler = Crawler::new();

    loop {
        if state.app_state.get("paused").unwrap().to_string() == "true" {
            // Run w/ a select on the shutdown signal otherwise we're stuck in an
            // infinite loop
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                    continue
                }
                _ = shutdown_rx.recv() => {
                    log::info!("🛑 Shutting down worker");
                    return;
                }
            }
        }

        let next_cmd = tokio::select! {
            res = queue.recv() => res,
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down worker");
                return;
            }
        };

        if let Some(cmd) = next_cmd {
            match cmd {
                Command::Fetch(task) => {
                    tokio::spawn(_handle_fetch(state.clone(), crawler.clone(), task.clone()));
                }
            }
        }
    }
}
