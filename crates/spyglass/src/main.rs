#[macro_use]
extern crate rocket;

use tokio::signal;
use tokio::sync::{broadcast, mpsc};
use tracing_subscriber::EnvFilter;

use libspyglass::models::crawl_queue;
use libspyglass::state::AppState;
use libspyglass::task::{self, AppShutdown};
use shared::config::Config;

mod api;
mod importer;

use crate::api::start_api;

#[tokio::main]
async fn main() {
    let file_appender = tracing_appender::rolling::daily(Config::logs_dir(), "server.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(
            EnvFilter::default()
                .add_directive(tracing::Level::INFO.into())
                .add_directive("tantivy=WARN".parse().unwrap())
                .add_directive("rocket=WARN".parse().unwrap()),
        )
        .init();

    // Initialize/Load user preferences
    let state = AppState::new().await;
    // TODO: Implement user-friendly start-up wizard
    // if state.config.user_settings.run_wizard {
    //     // Import data from Firefox
    //     // TODO: Ask user what browser/profiles to import on first startup.
    //     let importer = FirefoxImporter::new(&state.config);
    //     let _ = importer.import(&state).await;
    // }

    // Initialize crawl_queue, set all in-flight tasks to queued.
    crawl_queue::reset_processing(&state.db).await;

    // Check lenses for updates
    // Figure out how to handle wildcard domains
    for (_, lens) in state.config.lenses.iter() {
        for domain in lens.domains.iter() {
            let _ = crawl_queue::enqueue(
                &state.db,
                &format!("https://{}", domain),
                &state.config.user_settings,
            )
            .await;
        }
    }

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
