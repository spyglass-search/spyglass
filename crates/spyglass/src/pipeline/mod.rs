pub mod collector;
pub mod default_pipeline;
pub mod parser;

use crate::search::lens;
use crate::state::AppState;
use crate::task::AppShutdown;
use crate::task::CrawlTask;
use entities::models::crawl_queue;
use entities::sea_orm::DatabaseConnection;
use shared::config::Config;
use shared::config::PipelineConfiguration;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use tokio::sync::{broadcast, mpsc};

// The pipeline context is a context object that is passed between
// all stages of the pipeline. This allows later stages in the pipeline
// to change behavior based on previous stages.
#[derive(Debug, Clone)]
pub struct PipelineContext {
    pipeline_name: String,
    metadata: HashMap<String, String>,
    db: DatabaseConnection,
}

impl PipelineContext {
    // Constructor for the pipeline context
    fn new(name: &str, db: DatabaseConnection) -> Self {
        Self {
            pipeline_name: name.to_owned(),
            metadata: HashMap::new(),
            db,
        }
    }
}

// Commands that can be sent to pipelines
#[derive(Debug, Clone)]
pub enum PipelineCommand {
    ProcessUrl(String, CrawlTask),
}

// General pipeline initialize function. This function will read the lenses and pipelines
// and spawn new tasks to handel each configured pipeline. This will also read and pipeline
// commands and forward them to the appropriate task.
pub async fn initialize_pipelines(
    app_state: AppState,
    config: Config,
    mut general_pipeline_queue: mpsc::Receiver<PipelineCommand>,
    shutdown_tx: broadcast::Sender<AppShutdown>,
) {
    let mut shutdown_rx = shutdown_tx.subscribe();

    // Yes probably should do some error handling, but not really needed. No pipelines
    // just means not tasks to send.
    let _ = lens::read_lenses(&app_state, &config).await;
    let _ = read_pipelines(&app_state, &config).await;

    // Grab all pipelines
    let configured_pipelines: HashSet<String> = app_state
        .lenses
        .iter()
        .filter(|entry| entry.value().pipeline.as_ref().is_some())
        .map(|entry| entry.value().pipeline.as_ref().unwrap().clone())
        .collect();

    let mut pipelines: HashMap<String, PipelineConfiguration> = HashMap::new();
    for entry in app_state.pipelines.iter() {
        pipelines.insert(entry.key().clone(), entry.value().clone());
    }

    let mut pipeline_tx_map = HashMap::new();
    for pipeline in configured_pipelines {
        log::info!("Initializing Pipeline {:?}!", pipeline);
        if pipelines.contains_key(&pipeline) {
            let (pipeline_cmd_tx, pipeline_cmd_rx) = mpsc::channel(16);
            pipeline_tx_map.insert(pipeline.clone(), pipeline_cmd_tx);

            tokio::spawn(default_pipeline::pipeline_loop(
                app_state.clone(),
                config.clone(),
                pipeline.clone(),
                pipelines.get(&pipeline).unwrap().clone(),
                pipeline_cmd_rx,
                shutdown_tx.subscribe(),
            ));
        } else {
            log::error!(
                "Lens configured with a pipeline that is not configured {:?}",
                pipeline
            );
        }
    }

    loop {
        let next_thing = tokio::select! {
            res = general_pipeline_queue.recv() => {
                log::debug!("Received request to top level pipeline");
                res

            }
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down top level pipeline");
                return;
            }
        };

        if let Some(pipeline_cmd) = next_thing {
            match pipeline_cmd {
                PipelineCommand::ProcessUrl(pipeline, task) => {
                    let tx = pipeline_tx_map.get(&pipeline);
                    match tx {
                        Some(sender) => {
                            let cmd = PipelineCommand::ProcessUrl(
                                pipeline.clone(),
                                CrawlTask { id: task.id },
                            );
                            if sender.send(cmd).await.is_err() {
                                log::error!(
                                    "Unable to forward message to pipeline {:?}",
                                    &pipeline
                                );
                            }
                        }
                        None => {
                            log::error!("No pipeline configuration found for pipeline {:?}, failing crawl id: {}", &pipeline, task.id);
                            fail_crawl_cmd(&app_state, task.id).await;
                        }
                    }
                }
            }
        }
    }
}

// Helper function used to set any crawl failures with the status of failed.
pub async fn fail_crawl_cmd(state: &AppState, task_uid: i64) {
    // mark crawl as failed
    if let Err(e) =
        crawl_queue::mark_done(&state.db, task_uid, crawl_queue::CrawlStatus::Failed).await
    {
        log::error!("Unable to mark task as failed: {}", e);
    }
}

/// Read pipelines into the AppState
pub async fn read_pipelines(state: &AppState, config: &Config) -> anyhow::Result<()> {
    log::info!("Reading pipelines");
    state.pipelines.clear();

    let pipelines_dir = config.pipelines_dir();

    for entry in (fs::read_dir(pipelines_dir)?).flatten() {
        let path = entry.path();
        log::info!("Path ::: {:?}", path);
        if path.is_file() && path.extension().unwrap_or_default() == "ron" {
            log::info!("{:?}", &path);
            if let Ok(file_contents) = fs::read_to_string(path) {
                log::info!("{:?}", &file_contents);
                match ron::from_str::<PipelineConfiguration>(&file_contents) {
                    Err(err) => log::error!(
                        "Unable to load pipeline configuration {:?}: {}",
                        entry.path(),
                        err
                    ),
                    Ok(pipeline) => {
                        state.pipelines.insert(pipeline.kind.clone(), pipeline);
                    }
                }
            }
        }
    }

    Ok(())
}
