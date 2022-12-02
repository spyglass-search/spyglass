use entities::models::crawl_queue;
use tokio::sync::mpsc;

use super::{CrawlTask, WorkerCommand};
use crate::pipeline::PipelineCommand;
use crate::state::AppState;

// Check for new jobs in the crawl queue and add them to the worker queue.
#[tracing::instrument(skip(state, queue))]
pub async fn check_for_jobs(state: &AppState, queue: &mpsc::Sender<WorkerCommand>) -> bool {
    match crawl_queue::dequeue(&state.db, state.user_settings.clone()).await {
        Ok(Some(task)) => {
            match &task.pipeline {
                Some(pipeline) => {
                    if let Some(pipeline_tx) = state.pipeline_cmd_tx.lock().await.as_mut() {
                        log::debug!("Sending crawl task to pipeline");
                        let cmd = PipelineCommand::ProcessUrl(
                            pipeline.clone(),
                            CrawlTask { id: task.id },
                        );
                        if let Err(err) = pipeline_tx.send(cmd).await {
                            log::error!("Unable to send crawl task to pipeline {:?}", err);
                        }
                    }
                    true
                }
                None => {
                    // Send to worker
                    let cmd = WorkerCommand::Crawl { id: task.id };
                    if queue.send(cmd).await.is_err() {
                        log::error!("unable to send command to worker");
                    }
                    true
                }
            }
        }
        Ok(None) => {
            // nothing to do!
            false
        }
        Err(err) => {
            log::error!("Unable to dequeue jobs: {}", err.to_string());
            false
        }
    }
}
