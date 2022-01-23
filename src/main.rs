use simple_logger::SimpleLogger;
use tokio::sync::mpsc;

mod config;
mod crawler;
mod importer;
mod models;
mod scraper;
mod state;

use crate::crawler::Carto;
use crate::importer::FirefoxImporter;
use crate::models::CrawlQueue;
use crate::state::AppState;

#[derive(Debug)]
enum Command {
    Fetch(String),
}

#[tokio::main]
async fn main() {
    // Initialize logging system
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .with_module_level("sqlx::query", log::LevelFilter::Warn)
        .with_utc_timestamps()
        .init()
        .unwrap();

    // Import data from Firefox
    // TODO: Ask user what browser/profiles to import on first startup.
    let state = AppState::new().await;
    let importer = FirefoxImporter::new(&state.config);
    let _ = importer.import(&state).await;

    // Create a new channel with a capacity of at most 32.
    let (tx, mut rx) = mpsc::channel(32);

    // Main app loops
    let manager = tokio::spawn(async move {
        let state = AppState::new().await;
        let db = &state.conn;

        while let Some(cmd) = rx.recv().await {
            match cmd {
                Command::Fetch(url) => {
                    let _ = Carto::fetch(db, &url).await;
                }
            }
        }
    });

    let worker = tokio::spawn(async move {
        let state = AppState::new().await;
        let db = &state.conn;

        loop {
            // Do stuff
            if let Ok(Some(url)) = CrawlQueue::next(db).await {
                let cmd = Command::Fetch(url.to_string());
                // Send the GET request
                log::info!("sending fetch");
                if tx.send(cmd).await.is_err() {
                    eprintln!("connection task shutdown");
                    return;
                }
            } else {
                log::info!("nothing to crawl");
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });

    let _ = tokio::join!(manager, worker);
}
