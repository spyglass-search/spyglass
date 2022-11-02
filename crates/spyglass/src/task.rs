use notify::event::ModifyKind;
use notify::{EventKind, RecursiveMode, Watcher};
use tokio::sync::{broadcast, mpsc};

use shared::config::Config;

use crate::connection::{Connection, DriveConnection};
use crate::crawler::bootstrap;
use crate::search::lens::{load_lenses, read_lenses};
use crate::state::AppState;

mod manager;
mod worker;

#[derive(Debug, Clone)]
pub struct CrawlTask {
    pub id: i64,
}

#[derive(Clone, Debug)]
pub enum CollectTask {
    // Pull URLs from a CDX server
    Bootstrap {
        lens: String,
        seed_url: String,
        pipeline: Option<String>,
    },
    // Connects to an integration and discovers all the crawlable URIs
    ConnectionSync {
        connection_id: String,
    },
}

/// Tell the manager to schedule some tasks
#[derive(Clone, Debug)]
pub enum ManagerCommand {
    Collect(CollectTask),
    CheckForJobs,
}

/// Send tasks to the worker
#[derive(Clone, Debug)]
pub enum WorkerCommand {
    Collect(CollectTask),
    // Fetch, parses, & indexes a URI
    // TODO: Split this up so that this work can be spread out.
    Crawl { id: i64 },
    // Applies tag information to an URI
    Tag,
}

#[derive(Clone, Debug)]
pub enum AppPause {
    Pause,
    Run,
}

#[derive(Clone, Debug)]
pub enum AppShutdown {
    Now,
}

/// Manages the worker pool, scheduling tasks based on type/priority/etc.
#[tracing::instrument(skip_all)]
pub async fn manager_task(
    state: AppState,
    queue: mpsc::Sender<WorkerCommand>,
    manager_cmd_tx: mpsc::UnboundedSender<ManagerCommand>,
    mut manager_cmd_rx: mpsc::UnboundedReceiver<ManagerCommand>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    log::info!("manager started");
    let mut queue_check_interval = tokio::time::interval(std::time::Duration::from_millis(100));
    loop {
        tokio::select! {
            // Listen for manager level commands. This can be sent internally (i.e. CheckForJobs) or
            // externally (e.g. Collect)
            cmd = manager_cmd_rx.recv() => {
                if let Some(cmd) = cmd {
                    match cmd {
                        ManagerCommand::Collect(task) => {
                            log::debug!("collecting URIs");
                            if let Err(err) = queue.send(WorkerCommand::Collect(task)).await {
                                log::error!("Unable to send worker cmd: {}", err.to_string());
                            }
                        }
                        ManagerCommand::CheckForJobs => {
                            // log::debug!("checking for new jobs");
                            manager::check_for_jobs(&state, &queue).await
                        }
                    }
                }
            }
            // If we're not handling anything, continually poll for jobs.
            _ = queue_check_interval.tick() => {
                if let Err(err) = manager_cmd_tx.send(ManagerCommand::CheckForJobs) {
                    log::error!("Unable to send manager command: {}", err.to_string());
                }
            }
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down manager");
                manager_cmd_rx.close();
                return;
            }
        };
    }
}

/// Grabs a task
pub async fn worker_task(
    state: AppState,
    mut queue: mpsc::Receiver<WorkerCommand>,
    mut pause_rx: broadcast::Receiver<AppPause>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    log::info!("worker started");
    let mut is_paused = false;

    loop {
        // Run w/ a select on the shutdown signal otherwise we're stuck in an
        // infinite loop
        if is_paused {
            tokio::select! {
                res = pause_rx.recv() => {
                    if let Ok(AppPause::Run) = res {
                        is_paused = false;
                    }
                },
                _ = shutdown_rx.recv() => {
                    log::info!("🛑 Shutting down worker");
                    queue.close();
                    return;
                }
            };

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            continue;
        }

        let next_cmd = tokio::select! {
            res = queue.recv() => res,
            res = pause_rx.recv() => {
                if let Ok(AppPause::Pause) = res {
                    is_paused = true;
                }

                None
            },
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down worker");
                queue.close();
                return;
            }
        };

        if let Some(cmd) = next_cmd {
            log::debug!("handling command: {:?}", cmd);
            match cmd {
                WorkerCommand::Collect(task) => match task {
                    CollectTask::Bootstrap {
                        lens,
                        seed_url,
                        pipeline,
                    } => {
                        let state = state.clone();
                        tokio::spawn(async move {
                            if let Some(lens_config) = &state.lenses.get(&lens) {
                                worker::handle_bootstrap(&state, lens_config, &seed_url, pipeline)
                                    .await;
                            }
                        });
                    }
                    CollectTask::ConnectionSync { connection_id } => {
                        log::debug!("handling ConnectionSync for {}", connection_id);
                        let state = state.clone();
                        tokio::spawn(async move {
                            // TODO: dynamic dispatch based on connection id
                            match DriveConnection::new(&state).await {
                                Ok(mut conn) => {
                                    conn.sync(&state).await;
                                }
                                Err(err) => log::error!(
                                    "Unable to sync w/ connection: {} - {}",
                                    connection_id,
                                    err.to_string()
                                ),
                            }
                        });
                    }
                },
                WorkerCommand::Crawl { id } => {
                    tokio::spawn(worker::handle_fetch(state.clone(), CrawlTask { id }));
                }
                WorkerCommand::Tag => {}
            }
        }
    }
}

/// Watches the lens folder for new/updated lenses & reloads the metadata.
pub async fn lens_watcher(
    state: AppState,
    config: Config,
    mut pause_rx: broadcast::Receiver<AppPause>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    log::info!("👀 lens watcher started");

    let mut is_paused = false;
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    let mut watcher = notify::recommended_watcher(move |res| {
        futures::executor::block_on(async {
            tx.send(res).await.expect("Unable to send FS event");
        })
    })
    .expect("Unable to watch lens directory");

    let _ = watcher.watch(&config.lenses_dir(), RecursiveMode::Recursive);

    // Read + load lenses for the first time.
    let _ = read_lenses(&state, &config).await;
    load_lenses(state.clone()).await;

    loop {
        // Run w/ a select on the shutdown signal otherwise we're stuck in an
        // infinite loop
        if is_paused {
            tokio::select! {
                res = pause_rx.recv() => {
                    if let Ok(AppPause::Run) = res {
                        is_paused = false;
                    }
                },
                _ = shutdown_rx.recv() => {
                    log::info!("🛑 Shutting down lens watcher");
                    return;
                }
            };

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            continue;
        }

        let event = tokio::select! {
            res = rx.recv() => res,
            res = pause_rx.recv() => {
                if let Ok(AppPause::Pause) = res {
                    is_paused = true;
                }

                None
            },
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down lens watcher");
                return;
            }
        };

        if let Some(event) = event {
            match event {
                Ok(event) => {
                    let mut updated_lens = false;
                    for path in &event.paths {
                        if path.extension().unwrap_or_default() == "ron" {
                            updated_lens = true;
                        }
                    }

                    if updated_lens {
                        match event.kind {
                            EventKind::Create(_)
                            | EventKind::Modify(ModifyKind::Data(_))
                            | EventKind::Modify(ModifyKind::Name(_)) => {
                                let _ = read_lenses(&state, &config).await;
                                load_lenses(state.clone()).await;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => log::error!("watch error: {:?}", e),
            }
        }
    }
}
