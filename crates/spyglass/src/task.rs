use notify::event::ModifyKind;
use notify::{EventKind, RecursiveMode, Watcher};
use tokio::sync::{broadcast, mpsc};

use shared::config::Config;

use crate::crawler::Crawler;
use crate::search::lens::{load_lenses, read_lenses};
use crate::state::AppState;

mod manager;
mod worker;

#[derive(Debug, Clone)]
pub struct CrawlTask {
    pub id: i64,
}

/// Tell the manager to schedule some tasks
pub enum ManagerCommand {
    Collect,
    CheckForJobs,
}

/// Send tasks to the worker
#[derive(Clone, Debug)]
pub enum WorkerCommand {
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
            cmd = manager_cmd_rx.recv() => {
                if let Some(cmd) = cmd {
                    match cmd {
                        ManagerCommand::Collect => {
                            log::debug!("collecting uris for crawl queue");
                        }
                        ManagerCommand::CheckForJobs => {
                            log::debug!("checking for new jobs");
                            manager::check_for_jobs(&state, &queue).await
                        }
                    }
                }
            }
            _ = queue_check_interval.tick() => {
                if let Err(err) = manager_cmd_tx.send(ManagerCommand::CheckForJobs) {
                    log::error!("Unable to send manager command: {}", err.to_string());
                }
            }
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down manager");
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
    let crawler = Crawler::new();
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
                WorkerCommand::Crawl { id } => {
                    tokio::spawn(worker::handle_fetch(
                        state.clone(),
                        crawler.clone(),
                        CrawlTask { id },
                    ));
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
