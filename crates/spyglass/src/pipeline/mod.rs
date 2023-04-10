pub mod cache_pipeline;
pub mod collector;
pub mod default_pipeline;
pub mod parser;

use crate::search::lens;
use crate::state::AppState;
use crate::task::CrawlTask;
use entities::models::crawl_queue;
use shared::config::Config;
use shared::config::PipelineConfiguration;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use tokio::sync::mpsc;

// The pipeline context is a context object that is passed between
// all stages of the pipeline. This allows later stages in the pipeline
// to change behavior based on previous stages.
#[allow(dead_code)]
#[derive(Clone)]
pub struct PipelineContext {
    pipeline_name: String,
    metadata: HashMap<String, String>,
    state: AppState,
}

impl PipelineContext {
    // Constructor for the pipeline context
    fn new(name: &str, state: AppState) -> Self {
        Self {
            pipeline_name: name.to_owned(),
            metadata: HashMap::new(),
            state,
        }
    }
}

// Commands that can be sent to pipelines
#[derive(Debug, Clone)]
pub enum PipelineCommand {
    ProcessUrl(String, CrawlTask),
    ProcessCache(String, PathBuf),
}

// General pipeline initialize function. This function will read the lenses and pipelines
// and spawn new tasks to handel each configured pipeline. This will also read and pipeline
// commands and forward them to the appropriate task.
pub async fn initialize_pipelines(
    app_state: AppState,
    config: Config,
    mut general_pipeline_queue: mpsc::Receiver<PipelineCommand>,
) {
    let mut shutdown_rx = app_state.shutdown_cmd_tx.lock().await.subscribe();
    // Yes probably should do some error handling, but not really needed. No pipelines
    // just means not tasks to send.
    let lens_map = lens::read_lenses(&config).await.unwrap_or_default();
    let _ = read_pipelines(&app_state, &config).await;

    // Grab all pipelines
    let configured_pipelines: HashSet<String> = lens_map
        .iter()
        .filter_map(|entry| entry.value().pipeline.clone())
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
                pipelines.get(&pipeline).expect("Expected pipeline").clone(),
                pipeline_cmd_rx,
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
                            log::warn!("No pipeline configuration found for pipeline {:?}, failing crawl id: {}", &pipeline, task.id);
                            fail_crawl_cmd(&app_state, task.id).await;
                        }
                    }
                }
                PipelineCommand::ProcessCache(lens, cache_file) => {
                    if let Some(lens_config) = app_state.lenses.get(&lens) {
                        cache_pipeline::process_update(
                            app_state.clone(),
                            &lens_config,
                            cache_file,
                            false,
                        )
                        .await;
                    }
                }
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

// Helper function used to set any crawl failures with the status of failed.
pub async fn fail_crawl_cmd(state: &AppState, task_uid: i64) {
    // mark crawl as failed
    crawl_queue::mark_failed(&state.db, task_uid, false).await;
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
