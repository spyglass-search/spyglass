use super::PipelineContext;
use crate::crawler::{CrawlResult, Crawler};
use url::Url;

pub struct DefaultParser {
    crawler: Crawler,
}
pub struct ParseResult {
    pub content: CrawlResult,
}

// The default parser currently delegates to the crawler to handle scraping of the page.
// It is planned to have multiple parsers for various file formats and configuration options
// for data scraping
impl DefaultParser {
    pub async fn parse(
        &self,
        _context: &mut PipelineContext,
        crawl_result: &CrawlResult,
    ) -> Result<ParseResult, String> {
        if let Some(raw_content) = &crawl_result.content {
            let url = Url::parse(&crawl_result.url).expect("Invalid fetch URL");
            let scrape_result = self.crawler.scrape_page(&url, raw_content).await;
            return Result::Ok(ParseResult {
                content: scrape_result,
            });
        }
        Result::Err(String::from("Nope no parsing today"))
    }

    pub fn new() -> Self {
        Self {
            crawler: Crawler::new(),
        }
    }
}

impl Default for DefaultParser {
    fn default() -> Self {
        Self::new()
    }
}
