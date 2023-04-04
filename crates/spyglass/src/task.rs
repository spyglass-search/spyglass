use anyhow::anyhow;
use entities::models::crawl_queue::CrawlStatus;
use entities::models::{bootstrap_queue, connection, crawl_queue};
use entities::sea_orm::{sea_query::Expr, ColumnTrait, Condition, EntityTrait, QueryFilter};
use futures::StreamExt;
use notify::event::ModifyKind;
use notify::{EventKind, RecursiveMode, Watcher};
use shared::config::{Config, LensConfig, UserSettings, UserSettingsDiff};
use spyglass_rpc::{ModelDownloadStatusPayload, RpcEvent, RpcEventType};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use strum::IntoEnumIterator;
use tokio::sync::{broadcast, mpsc};

use crate::connection::{api_id_to_label, load_connection};
use crate::crawler::bootstrap;
use crate::filesystem;
use crate::filesystem::extensions::AudioExt;
use crate::search::lens::{load_lenses, read_lenses};
use crate::search::Searcher;
use crate::state::AppState;
use crate::task::worker::FetchResult;
use diff::Diff;

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
        pipeline: Option<String>,
    },
    // Connects to an integration and discovers all the crawlable URIs
    ConnectionSync {
        api_id: String,
        account: String,
        is_first_sync: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupTask {
    pub missing_docs: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub enum UserSettingsChange {
    SettingsChanged(UserSettings),
}

/// Tell the manager to schedule some tasks
#[derive(Clone, Debug)]
pub enum ManagerCommand {
    Collect(CollectTask),
    CheckForJobs,
    /// General database cleanup command that sends a worker
    /// task request for cleanup
    CleanupDatabase(CleanupTask),
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
    // Startup filesystem watcher
    filesystem::configure_watcher(state.clone()).await;

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
                                queue_check_interval = tokio::time::interval(Duration::from_millis(256));
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

/// Manages changes to the user's settings
#[tracing::instrument(skip_all)]
pub async fn config_task(mut state: AppState) {
    log::info!("Starting Configuration Watcher");

    let mut shutdown_rx = state.shutdown_cmd_tx.lock().await.subscribe();
    let mut config_rx = state.config_cmd_tx.lock().await.subscribe();

    loop {
        tokio::select! {
            cmd = config_rx.recv() => {
                if let Ok(UserSettingsChange::SettingsChanged(new_settings)) = cmd {
                    log::debug!("User Settings Updated {:?}", new_settings);
                    let old_config = state.user_settings.load_full();

                    if Config::save_user_settings(&new_settings).is_ok() {
                        state.reload_config();
                        let diff = new_settings.diff(&old_config);
                        // Process any new added paths
                        process_filesystem_changes(&state, &diff).await;
                        // Audio transcriptions enabled?
                        if new_settings.audio_settings.enable_audio_transcription {
                            // Do we already have this model?
                            let model_path = state.config.model_dir().join("whisper.base.en.bin");
                            if !model_path.exists() {
                                // Spawn a background task to download and send progress updates to
                                // any listening clients
                                let state_clone = state.clone();
                                tokio::spawn(async move {
                                    let _ = download_model(&state_clone, "Audio Transcription Model", model_path).await;
                                    // Once we're done downloading the model, recrawl any audio files
                                    let audio_exts = AudioExt::iter().map(|x| x.to_string()).collect::<Vec<String>>();
                                    let mut condition = Condition::any();
                                    for ext in audio_exts {
                                        condition = condition.add(crawl_queue::Column::Url.ends_with(&format!(".{}", ext)));
                                    }

                                    let _ = crawl_queue::Entity::update_many()
                                        .col_expr(crawl_queue::Column::Status, Expr::value(CrawlStatus::Queued))
                                        .filter(condition)
                                        .exec(&state_clone.db)
                                        .await;
                                });
                            }
                        }
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down configuration watcher");
                return;
            }
        };
    }
}

/// Downloads a model from our assets S3 bucket
async fn download_model(
    state: &AppState,
    model_name: &str,
    model_path: PathBuf,
) -> anyhow::Result<()> {
    // Currently we only have the audio model :)
    match reqwest::get(shared::constants::WHISPER_MODEL).await {
        Ok(res) => {
            let total_size = res.content_length().expect("Unable to get content length");
            let mut file = File::create(model_path).or(Err(anyhow!("Failed to create file")))?;

            let mut downloaded: u64 = 0;
            let mut stream = res.bytes_stream();

            // Download model in chunks, writing to model path.

            // Set the last update to some time in the past so we immediately send an update
            let mut last_update = std::time::Instant::now() - std::time::Duration::from_secs(100);
            while let Some(item) = stream.next().await {
                let chunk = item.or(Err(anyhow!("Error while downloading file")))?;
                file.write_all(&chunk)
                    .or(Err(anyhow!("Error while writing to file")))?;

                let new = std::cmp::min(downloaded + (chunk.len() as u64), total_size);
                downloaded = new;

                // Send an update to client every ~10 secs
                if last_update.elapsed().as_secs() > 10 {
                    let percent = ((downloaded as f32 / total_size as f32) * 100f32) as u8;
                    state
                        .publish_event(&RpcEvent {
                            event_type: RpcEventType::ModelDownloadStatus,
                            payload: serde_json::to_string(
                                &ModelDownloadStatusPayload::InProgress {
                                    model_name: model_name.into(),
                                    percent,
                                },
                            )
                            .unwrap_or_default(),
                        })
                        .await;
                    last_update = std::time::Instant::now();
                }
            }

            // finished download!
            state
                .publish_event(&RpcEvent {
                    event_type: RpcEventType::ModelDownloadStatus,
                    payload: serde_json::to_string(&ModelDownloadStatusPayload::Finished {
                        model_name: model_name.into(),
                    })
                    .unwrap_or_default(),
                })
                .await;
            Ok(())
        }
        Err(err) => {
            state
                .publish_event(&RpcEvent {
                    event_type: RpcEventType::ModelDownloadStatus,
                    payload: serde_json::to_string(&ModelDownloadStatusPayload::Error {
                        model_name: model_name.into(),
                        msg: err.to_string(),
                    })
                    .unwrap_or_default(),
                })
                .await;

            Ok(())
        }
    }
}

// Processes any needed filesystem configuration changes
async fn process_filesystem_changes(state: &AppState, diff: &UserSettingsDiff) {
    let fs_diff = &diff.filesystem_settings;
    if fs_diff.enable_filesystem_scanning.is_some()
        || !fs_diff.supported_extensions.0.is_empty()
        || !fs_diff.watched_paths.0.is_empty()
    {
        // fs configuration has changed update fs
        filesystem::configure_watcher(state.clone()).await;
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
    let updated_docs: Arc<AtomicI32> = Arc::new(AtomicI32::new(0i32));
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
                                pipeline,
                            } => {
                                log::debug!("handling CDXCollection for {}", lens);
                                let state = state.clone();
                                tokio::spawn(async move {
                                    if let Some(lens_config) = &state.lenses.get(&lens) {
                                        let _ = worker::handle_cdx_collection(&state, lens_config, pipeline)
                                            .await;
                                    }
                                });
                            }
                            CollectTask::ConnectionSync { api_id, account, is_first_sync } => {
                                log::debug!("handling ConnectionSync for {}", api_id);
                                let state = state.clone();
                                tokio::spawn(async move {
                                    match connection::get_by_id(&state.db, &api_id, &account).await {
                                        Ok(Some(connection)) => {
                                            match load_connection(&state, &api_id, &account).await {
                                                Ok(mut conn) => {
                                                    let last_sync = if is_first_sync { None } else { Some(connection.updated_at) };
                                                    conn.as_mut().sync(&state, last_sync).await;

                                                    let api_label = api_id_to_label(&api_id);
                                                    let postfix = if is_first_sync { "finished" } else { "updated" };
                                                    let payload = format!("{} ({}) {}", api_label, account, postfix);

                                                    state.publish_event(&RpcEvent {
                                                        event_type: RpcEventType::ConnectionSyncFinished,
                                                        payload,
                                                    }).await;
                                                }
                                                Err(err) => log::warn!("Unable to sync w/ connection: {account}@{api_id} - {err}"),
                                            }
                                        },
                                        Ok(None) => log::warn!("No connection {account}@{api_id}"),
                                        Err(err) => log::warn!("Unable to find connection {account}@{api_id} - {err}"),
                                    }
                                });
                            }
                        },
                        WorkerCommand::CleanupDatabase(cleanup_task) => {
                            let _ = worker::cleanup_database(&state, cleanup_task).await;
                        }
                        WorkerCommand::CommitIndex => {
                            let state = state.clone();
                            let num_updated = updated_docs.load(Ordering::Relaxed);
                            if num_updated > 0 {
                                log::debug!("committing {} new/updated docs in index", num_updated);
                                updated_docs.store(0, Ordering::Relaxed);
                                tokio::spawn(async move {
                                    let _ = Searcher::save(&state).await;
                                });
                            }
                        }
                        WorkerCommand::Crawl { id } => {
                            let state = state.clone();
                            let updated_docs = updated_docs.clone();
                            tokio::spawn(async move {
                                match worker::handle_fetch(state, CrawlTask { id }).await {
                                    FetchResult::New | FetchResult::Updated => {
                                        updated_docs.fetch_add(1, Ordering::Relaxed);
                                    }
                                    _ => {}
                                }
                            });
                        }
                        WorkerCommand::Recrawl { id } => {
                            let state = state.clone();
                            let updated_docs = updated_docs.clone();
                            tokio::spawn(async move {
                                match worker::handle_fetch(state.clone(), CrawlTask { id }).await {
                                    FetchResult::New | FetchResult::Updated => {
                                        updated_docs.fetch_add(1, Ordering::Relaxed);
                                    }
                                    FetchResult::NotFound => {
                                        // URL no longer exists, delete from index.
                                        log::debug!("URI not found, deleting from index");
                                        if let Err(err) = worker::handle_deletion(state.clone(), id).await {
                                            log::error!("Unable to delete {id}: {err}");
                                        }
                                    }
                                    FetchResult::Error(err) => {
                                        log::warn!("Unable to recrawl {} - {}", id, err);
                                    },
                                    FetchResult::Ignore => {}
                                }
                            });
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

        // Add a little delay before we grab the next task.
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
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
                            // Make sure it's a valid lens file before reloading
                            let updated_lens_config = std::fs::read_to_string(path)
                                .map(|s| ron::from_str::<LensConfig>(&s));

                            if let Ok(Ok(lens_config)) = updated_lens_config {
                                // remove from bootstrap queue so the config is rechecked.
                                let _ =
                                    bootstrap_queue::dequeue(&state.db, &lens_config.name).await;
                                updated_lens = true;
                            }
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
