use tokio::sync::{broadcast, mpsc};

use crate::crawler::Carto;
use crate::models::{CrawlQueue, DbPool};

#[derive(Debug)]
pub enum Command {
    Fetch(String),
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
    loop {
        // Do stuff
        if let Ok(_) = shutdown_rx.recv().await {
            log::info!("Shutting down manager");
            return;
        }

        if let Ok(Some(url)) = CrawlQueue::next(&pool).await {
            let cmd = Command::Fetch(url.to_string());
            // Send the GET request
            log::info!("sending fetch");
            if queue.send(cmd).await.is_err() {
                eprintln!("connection task shutdown");
                return;
            }
        } else {
            log::info!("nothing to crawl");
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
    loop {
        if let Ok(_) = shutdown_rx.recv().await {
            log::info!("Shutting down worker");
            return;
        }

        if let Some(cmd) = queue.recv().await {
            match cmd {
                Command::Fetch(url) => {
                    println!("fetching: {}", url);
                    let _ = Carto::fetch(&pool, &url).await;
                    // todo: parse + index document.
                }
            }
        }
    }
}
