#[macro_use]
extern crate html5ever;
#[macro_use]
extern crate rocket;

use simple_logger::SimpleLogger;
use tokio::signal;
use tokio::sync::{broadcast, mpsc};

mod api;
mod crawler;
mod importer;
mod models;
mod scraper;
mod search;
mod state;
mod task;
mod test;

use crate::api::start_api;
use crate::importer::FirefoxImporter;
use crate::models::crawl_queue;
use crate::state::AppState;
use crate::task::AppShutdown;

#[tokio::main]
async fn main() {
    // Initialize logging system
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .with_module_level("sqlx::query", log::LevelFilter::Warn)
        .with_utc_timestamps()
        .init()
        .unwrap();

    // Initialize/Load user preferences
    let state = AppState::new().await;
    if state.config.user_settings.run_wizard {
        // Import data from Firefox
        // TODO: Ask user what browser/profiles to import on first startup.
        let importer = FirefoxImporter::new(&state.config);
        let _ = importer.import(&state).await;
    }

    // Initialize crawl_queue, set all in-flight tasks to queued.
    crawl_queue::reset_processing(&state.db).await;

    // Startup manager, workers, & API server.
    let (tx, rx) = mpsc::channel(32);
    let (shutdown_tx, _) = broadcast::channel::<AppShutdown>(16);

    let manager_handle = tokio::spawn(task::manager_task(
        state.clone(),
        tx,
        shutdown_tx.subscribe(),
    ));

    let worker_handle = tokio::spawn(task::worker_task(
        state.clone(),
        rx,
        shutdown_tx.subscribe(),
    ));

    // Gracefully handle shutdowns
    let server = start_api(state.clone()).await;

    match signal::ctrl_c().await {
        Ok(()) => {
            log::warn!("Shutdown request received");
            server.notify();
            shutdown_tx.send(AppShutdown::Now).unwrap();
        }
        Err(err) => {
            log::error!("Unable to listen for shutdown signal: {}", err);
            server.notify();
            shutdown_tx.send(AppShutdown::Now).unwrap();
        }
    }

    let _ = tokio::join!(manager_handle, worker_handle);
}
