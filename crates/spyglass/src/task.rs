use notify::event::ModifyKind;
use notify::{EventKind, RecursiveMode, Watcher};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

use shared::config::Config;

use crate::connection::load_connection;
use crate::crawler::bootstrap;
use crate::search::lens::{load_lenses, read_lenses};
use crate::state::AppState;
use crate::task::worker::FetchResult;

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
        api_id: String,
        account: String,
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
    // Commit any changes that have been made to the index.
    CommitIndex,
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

    let mut queue_check_interval = tokio::time::interval(Duration::from_millis(100));
    let mut commit_check_interval = tokio::time::interval(Duration::from_secs(10));

    loop {
        tokio::select! {
            // Listen for manager level commands. This can be sent internally (i.e. CheckForJobs) or
            // externally (e.g. Collect)
            cmd = manager_cmd_rx.recv() => {
                if let Some(cmd) = cmd {
                    match cmd {
                        ManagerCommand::Collect(task) => {
                            if let Err(err) = queue.send(WorkerCommand::Collect(task)).await {
                                log::error!("Unable to send worker cmd: {}", err.to_string());
                            }
                        }
                        ManagerCommand::CheckForJobs => {
                            if !manager::check_for_jobs(&state, &queue).await {
                                // If no jobs were queue, sleep longer. This will keep
                                // CPU usage low when there is nothing going on and
                                // let the manager process jobs as quickly as possible
                                // if there are a lot of them.
                                queue_check_interval = tokio::time::interval(Duration::from_secs(5));
                                // first tick always completes immediately.
                                queue_check_interval.tick().await;
                            } else {
                                queue_check_interval = tokio::time::interval(Duration::from_millis(100));
                                // first tick always completes immediately.
                                queue_check_interval.tick().await;
                            }
                        }
                    }
                }
            }
            // Check for changes to the index & commit them
            _ = commit_check_interval.tick() => {
                let _ = queue.send(WorkerCommand::CommitIndex).await;
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
    let mut updated_docs = 0;

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
            match cmd {
                WorkerCommand::Collect(task) => match task {
                    CollectTask::Bootstrap {
                        lens,
                        seed_url,
                        pipeline,
                    } => {
                        log::debug!("handling Bootstrap for {} - {}", lens, seed_url);
                        let state = state.clone();
                        tokio::spawn(async move {
                            if let Some(lens_config) = &state.lenses.get(&lens) {
                                worker::handle_bootstrap(&state, lens_config, &seed_url, pipeline)
                                    .await;
                            }
                        });
                    }
                    CollectTask::ConnectionSync { api_id, account } => {
                        log::debug!("handling ConnectionSync for {}", api_id);
                        let state = state.clone();
                        tokio::spawn(async move {
                            match load_connection(&state, &api_id, &account).await {
                                Ok(mut conn) => {
                                    conn.as_mut().sync(&state).await;
                                }
                                Err(err) => log::error!(
                                    "Unable to sync w/ connection: {} - {}",
                                    api_id,
                                    err.to_string()
                                ),
                            }
                        });
                    }
                },
                WorkerCommand::CommitIndex => {
                    let state = state.clone();
                    if updated_docs > 0 {
                        log::debug!("committing {} new/updated docs in index", updated_docs);
                        updated_docs = 0;
                        tokio::spawn(async move {
                            match state.index.writer.lock() {
                                Ok(mut writer) => {
                                    let _ = writer.commit();
                                }
                                Err(err) => {
                                    log::debug!(
                                        "Unable to acquire lock on index writer: {}",
                                        err.to_string()
                                    )
                                }
                            }
                        });
                    }
                }
                WorkerCommand::Crawl { id } => {
                    if let Ok(fetch_result) =
                        tokio::spawn(worker::handle_fetch(state.clone(), CrawlTask { id })).await
                    {
                        match fetch_result {
                            FetchResult::New | FetchResult::Updated => updated_docs += 1,
                            _ => {}
                        }
                    }
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
