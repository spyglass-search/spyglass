use sea_orm::prelude::*;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use tantivy::IndexWriter;
use tokio::sync::{broadcast, mpsc};
use url::Url;

use crate::config::Config;
use crate::crawler::Crawler;
use crate::models::{crawl_queue, indexed_document};
use crate::search::Searcher;

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
pub async fn manager_task(
    db: DatabaseConnection,
    config: Config,
    queue: mpsc::Sender<Command>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    log::info!("manager started");
    loop {
        // tokio::select allows us to listen to a shutdown message while
        // also processing queue tasks.
        let next_url = tokio::select! {
            res = crawl_queue::dequeue(&db, config.user_settings.domain_crawl_limit.clone()) => res.unwrap(),
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
            update.update(&db).await.unwrap();

            // Send to worker
            let cmd = Command::Fetch(CrawlTask { id: task_id });
            if queue.send(cmd).await.is_err() {
                eprintln!("unable to send command to worker");
                return;
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

/// Grabs a task
pub async fn worker_task(
    db: DatabaseConnection,
    config: Config,
    mut index: IndexWriter,
    mut queue: mpsc::Receiver<Command>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    log::info!("worker started");
    loop {
        let next_cmd = tokio::select! {
            res = queue.recv() => res,
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down worker");
                return;
            }
        };

        if let Some(cmd) = next_cmd {
            log::info!("received cmd: {:?}", cmd);
            match cmd {
                Command::Fetch(crawl) => {
                    let result = Crawler::fetch(&db, crawl.id).await;
                    // mark crawl as finished
                    crawl_queue::mark_done(&db, crawl.id).await.unwrap();

                    match result {
                        Ok(Some(crawl_result)) => {
                            // Add links found to crawl queue
                            for link in crawl_result.links.iter() {
                                crawl_queue::enqueue(&db, link, &config.user_settings).await.unwrap();
                            }

                            // Add / update search index w/ crawl result.
                            if let Some(content) = crawl_result.content {
                                let url = crawl_result.url;

                                let existing = indexed_document::Entity::find()
                                    .filter(indexed_document::Column::Url.eq(url.as_str()))
                                    .one(&db)
                                    .await
                                    .unwrap();

                                // Delete old document, if any.
                                if let Some(doc) = &existing {
                                    Searcher::delete(&mut index, &doc.doc_id).unwrap();
                                }

                                // Add document to index
                                let doc_id = Searcher::add_document(
                                    &mut index,
                                    &crawl_result.title.unwrap_or_default(),
                                    &crawl_result.description.unwrap_or_default(),
                                    &url,
                                    &content,
                                )
                                .unwrap();

                                // Update/create index reference in our database
                                let indexed = if let Some(doc) = existing {
                                    let mut update: indexed_document::ActiveModel = doc.into();
                                    update.doc_id = Set(doc_id);
                                    update.updated_at = Set(chrono::Utc::now());
                                    update
                                } else {
                                    let parsed = Url::parse(&url).unwrap();
                                    indexed_document::ActiveModel {
                                        domain: Set(parsed.host_str().unwrap().to_string()),
                                        url: Set(url),
                                        doc_id: Set(doc_id),
                                        ..Default::default()
                                    }
                                };

                                indexed.save(&db).await.unwrap();
                            }
                        }
                        Err(err) => log::error!("Unable to crawl id: {} - {:?}", crawl.id, err),
                        _ => {}
                    }
                }
            }
        }
    }
}
