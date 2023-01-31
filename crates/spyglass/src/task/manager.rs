use entities::models::crawl_queue;
use tokio::sync::mpsc;

use super::{CrawlTask, WorkerCommand};
use crate::pipeline::PipelineCommand;
use crate::state::AppState;

// Check for new jobs in the crawl queue and add them to the worker queue.
#[tracing::instrument(skip(state, queue))]
pub async fn check_for_jobs(state: &AppState, queue: &mpsc::Sender<WorkerCommand>) -> bool {
    // Do we have any crawl tasks?
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
                    return true;
                }
                None => {
                    // Send to worker
                    let cmd = WorkerCommand::Crawl { id: task.id };
                    if queue.send(cmd).await.is_err() {
                        log::error!("unable to send command to worker");
                    }
                    return true;
                }
            }
        }
        Err(err) => {
            log::error!("Unable to dequeue jobs: {}", err.to_string());
            return false;
        }
        _ => {}
    }

    // No crawl tasks, check for recrawl tasks
    match crawl_queue::dequeue_recrawl(&state.db, &state.user_settings).await {
        Ok(Some(task)) => {
            // Send to worker
            let cmd = WorkerCommand::Recrawl { id: task.id };
            if queue.send(cmd).await.is_err() {
                log::error!("unable to send command to worker");
            }
            return true;
        }
        Err(err) => {
            log::error!("Unable to dequeue_recrawl jobs: {}", err.to_string());
            return false;
        }
        _ => {}
    }

    false
}

#[cfg(test)]
mod test {
    use tokio::sync::mpsc;

    use super::check_for_jobs;
    use crate::{state::AppState, task::WorkerCommand};
    use entities::models::crawl_queue::{self, CrawlStatus, CrawlType};
    use entities::sea_orm::{ActiveModelTrait, Set};
    use entities::test::setup_test_db;

    #[tokio::test]
    async fn test_check_for_jobs() {
        let db = setup_test_db().await;
        let state = AppState::builder().with_db(db.clone()).build();

        // Insert dummy job
        let task = crawl_queue::ActiveModel {
            url: Set("https://example.com".to_owned()),
            domain: Set("example.com".to_owned()),
            crawl_type: Set(CrawlType::Normal),
            status: Set(CrawlStatus::Queued),
            ..Default::default()
        };
        let mut saved = task.save(&db).await.expect("Unable to save dummy task");

        let (sender, mut recv) = mpsc::channel(10);
        let has_job = check_for_jobs(&state, &sender).await;
        assert!(has_job);

        let message = recv.recv().await.expect("no WorkerCommand in channel");
        assert_eq!(
            message,
            WorkerCommand::Crawl {
                id: saved.id.take().unwrap_or_default()
            }
        );
    }

    #[tokio::test]
    async fn test_check_for_jobs_recrawl() {
        let db = setup_test_db().await;
        let state = AppState::builder().with_db(db.clone()).build();

        // Insert dummy job
        let one_day_ago = chrono::Utc::now() - chrono::Duration::days(1);
        let task = crawl_queue::ActiveModel {
            url: Set("file:///tmp/test.txt".to_owned()),
            domain: Set("localhost".to_owned()),
            crawl_type: Set(CrawlType::Normal),
            status: Set(CrawlStatus::Completed),
            created_at: Set(one_day_ago),
            updated_at: Set(one_day_ago),
            ..Default::default()
        };
        let _ = task.save(&db).await.expect("Unable to save dummy task");

        let two_day_ago = chrono::Utc::now() - chrono::Duration::days(2);
        let task = crawl_queue::ActiveModel {
            url: Set("file:///tmp/this_one.txt".to_owned()),
            domain: Set("localhost".to_owned()),
            crawl_type: Set(CrawlType::Normal),
            status: Set(CrawlStatus::Completed),
            created_at: Set(two_day_ago),
            updated_at: Set(two_day_ago),
            ..Default::default()
        };
        let mut saved = task.save(&db).await.expect("Unable to save dummy task");

        let (sender, mut recv) = mpsc::channel(10);
        let has_job = check_for_jobs(&state, &sender).await;
        assert!(has_job);

        // Should return the ID of the latest task.
        let message = recv.recv().await.expect("no WorkerCommand in channel");
        assert_eq!(
            message,
            WorkerCommand::Recrawl {
                id: saved.id.take().unwrap_or_default()
            }
        );
    }
}
