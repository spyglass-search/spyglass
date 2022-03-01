use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};

use tantivy::IndexWriter;
use tokio::sync::{broadcast, mpsc};

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
    queue: mpsc::Sender<Command>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    log::info!("manager started");
    loop {
        // tokio::select allows us to listen to a shutdown message while
        // also processing queue tasks.
        let next_url = tokio::select! {
            res = crawl_queue::Entity::find()
                .filter(crawl_queue::Column::Status.contains(&crawl_queue::CrawlStatus::Queued.to_string()))
                .order_by_asc(crawl_queue::Column::CreatedAt)
                .one(&db) => res.unwrap(),
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down manager");
                return;
            }
        };

        if let Some(task) = next_url {
            let cmd = Command::Fetch(CrawlTask { id: task.id });
            // Send the GET request
            log::info!("sending fetch");
            if queue.send(cmd).await.is_err() {
                eprintln!("connection task shutdown");
                return;
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

/// Grabs a task
pub async fn worker_task(
    db: DatabaseConnection,
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
                Command::Fetch(crawl) => match Crawler::fetch(&db, crawl.id).await {
                    Ok(Some(crawl_result)) => {
                        if let Some(content) = crawl_result.content {
                            let url = crawl_result.url.unwrap_or_default();

                            // Add / Update search index
                            let existing = indexed_document::Entity::find()
                                .filter(indexed_document::Column::Url.eq(url.as_str()))
                                .one(&db)
                                .await
                                .unwrap();

                            if let Some(doc) = existing {
                                // Delete old document
                                Searcher::delete(&index, doc.doc_id.to_string());
                                "verify delete + update"
                            }

                            match Searcher::add_document(
                                &mut index,
                                &crawl_result.title.unwrap_or_default(),
                                &crawl_result.description.unwrap_or_default(),
                                &url,
                                &content,
                            ) {
                                Ok(()) => log::info!("indexed document"),
                                Err(_) => log::error!("Unable to index crawl id: {}", crawl.id),
                            }

                            let new_doc = indexed_document::ActiveModel {
                                url: sea_orm::Set(url),
                                ..Default::default()
                            };

                            indexed_document::Entity::insert(new_doc)
                                .exec(&db)
                                .await
                                .unwrap();
                        }
                    }
                    Err(err) => log::error!("Unable to crawl id: {} - {:?}", crawl.id, err),
                    _ => {}
                },
            }
        }
    }
}
