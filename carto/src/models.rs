use chrono::prelude::*;
use regex::Regex;
use url::Url;

#[derive(Debug)]
pub struct Place {
    pub id: i32,
    pub url: Url,
}

pub struct RobotsTxt {
    id: u64,
    domain: String,
    no_index: bool,
    disallow: Vec<Regex>,
    allow: Vec<Regex>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// When a URL was last fetched. Also used as a queue for the indexer to determine
/// what paths to index next.
#[derive(Debug)]
pub struct FetchHistory {
    /// Arbitrary id for this.
    id: u64,
    /// URL fetched.
    url: Url,
    /// Hash used to check for changes.
    hash: u64,
    /// HTTP status when last fetching this page.
    status: u8,
    /// Ignore this URL in the future.
    no_index: bool,
    /// When this was first added to our fetch history
    created_at: DateTime<Utc>,
    /// When this URL was last fetched.
    updated_at: DateTime<Utc>,
}