use entities::models::crawl_queue;
use tokio::sync::mpsc;

use super::{CrawlTask, WorkerCommand};
use crate::pipeline::PipelineCommand;
use crate::state::AppState;

// Check for new jobs in the crawl queue and add them to the worker queue.
#[tracing::instrument(skip(state, queue))]
pub async fn check_for_jobs(state: &AppState, queue: &mpsc::Sender<WorkerCommand>) {
    let mut prioritized_domains: Vec<String> = Vec::new();
    let mut prioritized_prefixes: Vec<String> = Vec::new();

    for entry in state.lenses.iter() {
        let value = entry.value();
        if value.pipeline.is_none() {
            prioritized_domains.extend(value.domains.clone());
            prioritized_prefixes.extend(value.urls.clone());
        }
    }

    match crawl_queue::dequeue(
        &state.db,
        state.user_settings.clone(),
        &prioritized_domains,
        &prioritized_prefixes,
    )
    .await
    {
        Ok(Some(task)) => {
            match &task.pipeline {
                Some(pipeline) => {
                    let mut pipeline_tx = state.pipeline_cmd_tx.lock().await;
                    match &mut *pipeline_tx {
                        Some(pipeline_tx) => {
                            log::debug!("Sending crawl task to pipeline");
                            let cmd = PipelineCommand::ProcessUrl(
                                pipeline.clone(),
                                CrawlTask { id: task.id },
                            );
                            if let Err(err) = pipeline_tx.send(cmd).await {
                                log::error!("Unable to send crawl task to pipeline {:?}", err);
                            }
                        }
                        None => {
                            log::error!("Unable to send crawl task to pipeline, no queue found");
                        }
                    }
                }
                None => {
                    // Send to worker
                    let cmd = WorkerCommand::Crawl { id: task.id };
                    if queue.send(cmd).await.is_err() {
                        log::error!("unable to send command to worker");
                    }
                }
            }
        }
        Ok(None) => {
            // nothing to do!
        }
        Err(err) => {
            log::error!("Unable to dequeue jobs: {}", err.to_string());
        }
    }
}
