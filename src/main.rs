#[macro_use]
extern crate html5ever;
#[macro_use]
extern crate rocket;

use rocket::Shutdown;
use simple_logger::SimpleLogger;
use tokio::signal;
use tokio::sync::{broadcast, mpsc};

mod api;
mod config;
mod crawler;
mod importer;
mod models;
mod scraper;
mod search;
mod state;

use crate::api::init_rocket;
use crate::crawler::Carto;
use crate::importer::FirefoxImporter;
use crate::models::CrawlQueue;
use crate::state::AppState;

#[derive(Debug)]
enum Command {
    Fetch(String),
}

#[derive(Clone, Debug)]
enum AppShutdown {
    Now,
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
    let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<AppShutdown>(16);

    // Main app loops
    let manager = tokio::spawn(async move {
        let state = AppState::new().await;
        let db = &state.conn;

        loop {
            if let Ok(_) = shutdown_rx.recv().await {
                log::info!("Shutting down manager");
                return;
            }

            if let Some(cmd) = rx.recv().await {
                match cmd {
                    Command::Fetch(url) => {
                        println!("fetching: {}", url);
                        // let _ = Carto::fetch(db, &url).await;
                    }
                }
            }
        }
    });

    let mut worker_shutdown = shutdown_tx.subscribe();
    let worker = tokio::spawn(async move {
        let state = AppState::new().await;
        let db = &state.conn;

        loop {
            // Do stuff
            if let Ok(_) = worker_shutdown.recv().await {
                log::info!("Shutting down worker");
                return;
            }

            // if let Ok(Some(url)) = CrawlQueue::next(db).await {
            //     let cmd = Command::Fetch(url.to_string());
            //     // Send the GET request
            log::info!("sending fetch");
            //     if tx.send(cmd).await.is_err() {
            //         eprintln!("connection task shutdown");
            //         return;
            //     }
            // } else {
            //     log::info!("nothing to crawl");
            // }

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });

    let server = init_rocket().await;
    match signal::ctrl_c().await {
        Ok(()) => {
            server.notify();
            shutdown_tx.send(AppShutdown::Now).unwrap();
        }
        Err(err) => {
            log::error!("Unable to listen for shutdown signal: {}", err);
            server.notify();
            shutdown_tx.send(AppShutdown::Now).unwrap();
        }
    }

    let _ = tokio::join!(manager, worker);
}
