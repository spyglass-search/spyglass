use super::PipelineContext;
use crate::crawler::{CrawlResult, Crawler};

pub trait PipelineCollector {
    fn collect(
        &self,
        context: &mut PipelineContext,
        task_id: i64,
    ) -> Result<CollectionResult, String>;
}

pub struct DefaultCollector {
    crawler: Crawler,
}

pub struct CollectionResult {
    pub content: CrawlResult,
}

// The default collector currently delegates to the crawler to process and normal fetch
// New collectors will be added to collect contents from web services.
impl DefaultCollector {
    pub async fn collect(
        &self,
        context: &mut PipelineContext,
        task_id: i64,
    ) -> Result<CollectionResult, String> {
        // Yes this is oddly familiar, since it is stolen from the _handle_fetch method in tasks. We will
        // need to merge the two concepts at a later time.
        let result = self
            .crawler
            .fetch_by_job(&context.state, task_id, false)
            .await;

        if let Ok(Some(crawl_result)) = result {
            return Result::Ok(CollectionResult {
                content: crawl_result,
            });
        }

        Err(String::from("No Result"))
    }
}

impl DefaultCollector {
    pub fn new() -> Self {
        Self {
            crawler: Crawler::new(),
        }
    }
}

impl Default for DefaultCollector {
    fn default() -> Self {
        Self::new()
    }
}
