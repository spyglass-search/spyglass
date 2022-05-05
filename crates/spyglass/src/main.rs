use std::io;
use tokio::signal;
use tokio::sync::{broadcast, mpsc};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

use libspyglass::state::AppState;
use libspyglass::task::{self, AppShutdown};
use migration::{Migrator, MigratorTrait};
use shared::config::Config;
use shared::models::crawl_queue;

mod api;
mod importer;

use crate::api::start_api_ipc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_appender = tracing_appender::rolling::daily(Config::logs_dir(), "server.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::registry()
        .with(
            EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
                .add_directive("tantivy=WARN".parse().unwrap()),
        )
        .with(fmt::Layer::new().with_writer(io::stdout))
        .with(fmt::Layer::new().with_ansi(false).with_writer(non_blocking));

    tracing::subscriber::set_global_default(subscriber).expect("Unable to set a global subscriber");
    LogTracer::init()?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("spyglass-backend")
        .build()
        .unwrap();

    // Initialize/Load user preferences
    let state = rt.block_on(AppState::new());

    // Run any migrations
    match rt.block_on(Migrator::up(&state.db, None)) {
        Ok(_) => {}
        Err(e) => {
            // Ruh-oh something went wrong
            log::error!("Unable to migrate database - {:?}", e);
            // Exit from app
            return Ok(());
        }
    }

    // Start IPC server
    let server = start_api_ipc(&state).expect("Unable to start IPC server");
    rt.block_on(start_backend(&state));
    server.close();

    Ok(())
}

async fn start_backend(state: &AppState) {
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
    // let server = start_api(state.clone()).await;
    match signal::ctrl_c().await {
        Ok(()) => {
            log::warn!("Shutdown request received");
            shutdown_tx.send(AppShutdown::Now).unwrap();
        }
        Err(err) => {
            log::error!("Unable to listen for shutdown signal: {}", err);
            shutdown_tx.send(AppShutdown::Now).unwrap();
        }
    }

    let _ = tokio::join!(manager_handle, worker_handle);
}
