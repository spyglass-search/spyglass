use notify::event::ModifyKind;
use notify::{EventKind, RecursiveMode, Watcher};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

use shared::config::Config;

use crate::connection::load_connection;
use crate::crawler::bootstrap;
use crate::filesystem;
use crate::search::lens::{load_lenses, read_lenses};
use crate::search::Searcher;
use crate::state::AppState;
use crate::task::worker::FetchResult;

mod manager;
pub mod worker;

#[derive(Debug, Clone)]
pub struct CrawlTask {
    pub id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CollectTask {
    BootstrapLens {
        lens: String,
    },
    // Pull URLs from a CDX server
    CDXCollection {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupTask {
    pub missing_docs: Vec<(String, String)>,
}

/// Tell the manager to schedule some tasks
#[derive(Clone, Debug)]
pub enum ManagerCommand {
    Collect(CollectTask),
    CheckForJobs,
    /// General database cleanup command that sends a worker
    /// task request for cleanup
    CleanupDatabase(CleanupTask),
    ToggleFilesystem(bool),
}

/// Send tasks to the worker
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkerCommand {
    /// Enqueues the URLs needed to start crawl.
    Collect(CollectTask),
    /// Commit any changes that have been made to the index.
    CommitIndex,
    /// Fetch, parses, & indexes a URI
    /// TODO: Split this up so that this work can be spread out.
    Crawl { id: i64 },
    /// Refetches, parses, & indexes a URI
    /// If the URI no longer exists (file moved, 404 etc), delete from index.
    Recrawl { id: i64 },
    /// Applies tag information to an URI
    Tag,
    /// Updates the document store for indexed document database table to
    /// cleanup inconsistencies
    CleanupDatabase(CleanupTask),
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
) {
    log::info!("manager started");

    let mut queue_check_interval = tokio::time::interval(Duration::from_millis(100));
    let mut commit_check_interval = tokio::time::interval(Duration::from_secs(10));
    let mut shutdown_rx = state.shutdown_cmd_tx.lock().await.subscribe();

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
                        },
                        ManagerCommand::CleanupDatabase(task) => {
                            if let Err(err) = queue.send(WorkerCommand::CleanupDatabase(task)).await {
                                log::error!("Unable to send worker cmd: {}", err.to_string());
                            }
                        },
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
                                queue_check_interval = tokio::time::interval(Duration::from_millis(50));
                                // first tick always completes immediately.
                                queue_check_interval.tick().await;
                            }
                        },
                        ManagerCommand::ToggleFilesystem (enabled) => {
                            let mut state = state.clone();
                            if let Ok(mut loaded_settings) = Config::load_user_settings() {
                                loaded_settings.filesystem_settings.enable_filesystem_scanning = enabled;
                                let _ = Config::save_user_settings(&loaded_settings);
                                state.user_settings = loaded_settings;
                            }

                            filesystem::configure_watcher(state).await;
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
    config: Config,
    mut queue: mpsc::Receiver<WorkerCommand>,
    mut pause_rx: broadcast::Receiver<AppPause>,
) {
    log::info!("worker started");
    let mut is_paused = false;
    let mut updated_docs = 0;
    let mut shutdown_rx = state.shutdown_cmd_tx.lock().await.subscribe();

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

        tokio::select! {
            res = queue.recv() => {
                if let Some(cmd) = res {
                    match cmd {
                        WorkerCommand::Collect(task) => match task {
                            CollectTask::BootstrapLens {
                                lens
                            } => {
                                log::debug!("Handling BootstrapLens for {}", lens);
                                let state = state.clone();
                                let config = config.clone();
                                tokio::spawn(async move {
                                    if let Some(lens_config) = &state.lenses.get(&lens) {
                                        worker::handle_bootstrap_lens(&state, &config, lens_config)
                                            .await;
                                    } else {
                                        log::error!("Unable to find requested lens {:?}, lens list {:?}", lens, state.lenses);
                                    }
                                });
                            },
                            CollectTask::CDXCollection {
                                lens,
                                seed_url,
                                pipeline,
                            } => {
                                log::debug!("handling CDXCollection for {} - {}", lens, seed_url);
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
                        WorkerCommand::CleanupDatabase(cleanup_task) => {
                            let _ = worker::cleanup_database(&state, cleanup_task).await;
                        }
                        WorkerCommand::CommitIndex => {
                            let state = state.clone();
                            if updated_docs > 0 {
                                log::debug!("committing {} new/updated docs in index", updated_docs);
                                updated_docs = 0;
                                tokio::spawn(async move {
                                    let _ = Searcher::save(&state).await;
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
                        WorkerCommand::Recrawl { id } => {
                            if let Ok(fetch_result) = tokio::spawn(worker::handle_fetch(state.clone(), CrawlTask { id })).await
                            {
                                match fetch_result {
                                    FetchResult::New | FetchResult::Updated => updated_docs += 1,
                                    FetchResult::NotFound => {
                                        // URL no longer exists, delete from index.
                                        log::debug!("URI not found, deleting from index");
                                        let _ = tokio::spawn(worker::handle_deletion(state.clone(), id)).await;
                                    }
                                    FetchResult::Error(err) => {
                                        log::warn!("Unable to recrawl {} - {}", id, err);
                                    },
                                    FetchResult::Ignore => {}
                                }
                            }
                        }
                        WorkerCommand::Tag => {}
                    }
                }
            },
            res = pause_rx.recv() => {
                if let Ok(AppPause::Pause) = res {
                    is_paused = true;
                }
            },
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down worker");
                queue.close();
                return;
            }
        };
    }
}

/// Watches the lens folder for new/updated lenses & reloads the metadata.
pub async fn lens_watcher(
    state: AppState,
    config: Config,
    mut pause_rx: broadcast::Receiver<AppPause>,
) {
    log::info!("👀 lens watcher started");
    let mut shutdown_rx = state.shutdown_cmd_tx.lock().await.subscribe();

    let mut is_paused = false;
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    let mut watcher = notify::recommended_watcher(move |res| {
        futures::executor::block_on(async {
            if !tx.is_closed() {
                if let Err(err) = tx.send(res).await {
                    log::error!("fseventwatcher channel error: {}. If we're in shutdown mode, nothing to worry about.", err.to_string());
                }
            }
        })
    })
    .expect("Unable to watch lens directory");

    let _ = watcher.watch(&config.lenses_dir(), RecursiveMode::Recursive);

    // Read + load lenses for the first time.
    let lens_map = read_lenses(&config).await.unwrap_or_default();

    load_lenses(&lens_map, state.clone()).await;

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
                                if let Ok(lens_map) = read_lenses(&config).await {
                                    load_lenses(&lens_map, state.clone()).await;
                                }
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
