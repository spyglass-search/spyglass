use tantivy::IndexWriter;
use tokio::sync::{broadcast, mpsc};

use crate::crawler::Crawler;
use crate::models::{CrawlQueue, DbPool};
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
    pool: DbPool,
    queue: mpsc::Sender<Command>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    log::info!("manager started");
    loop {
        let next_url = tokio::select! {
            res = CrawlQueue::next(&pool) => res.unwrap(),
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down manager");
                return;
            }
        };

        if let Some(crawl_task) = next_url {
            let cmd = Command::Fetch(crawl_task.clone());
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
    pool: DbPool,
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
                Command::Fetch(crawl) => match Crawler::fetch(&pool, crawl.id).await {
                    Ok(Some(crawl_result)) => {
                        if let Some(content) = crawl_result.content {
                            match Searcher::add_document(&mut index, "test document", &content) {
                                Ok(()) => log::info!("indexed document"),
                                Err(_) => log::error!("Unable to index crawl id: {}", crawl.id),
                            }
                        }
                    }
                    Err(err) => log::error!("Unable to crawl id: {} - {:?}", crawl.id, err),
                    _ => {}
                },
            }
        }
    }
}
