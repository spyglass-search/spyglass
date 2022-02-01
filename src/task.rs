use tokio::sync::{broadcast, mpsc};

use crate::crawler::Crawler;
use crate::models::{CrawlQueue, DbPool};

#[derive(Debug)]
pub enum Command {
    Fetch(String, bool),
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
                log::info!("Shutting down manager");
                return;
            }
        };

        if let Some(url) = next_url {
            let cmd = Command::Fetch(url.to_string(), false);
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
    mut queue: mpsc::Receiver<Command>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    log::info!("worker started");
    loop {
        let next_cmd = tokio::select! {
            res = queue.recv() => res,
            _ = shutdown_rx.recv() => {
                log::info!("Shutting down worker");
                return;
            }
        };

        if let Some(cmd) = next_cmd {
            log::info!("received cmd: {:?}", cmd);
            match cmd {
                Command::Fetch(url, force_crawl) => {
                    println!("fetching: {}", url);
                    let _ = Crawler::fetch(&pool, &url).await;
                    // todo: parse + index document.
                }
            }
        }
    }
}
