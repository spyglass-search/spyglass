use rocket::serde::Serialize;
use crate::models::CrawlQueue;

#[derive(Serialize)]
pub struct ListQueue {
    pub queue: Vec<CrawlQueue>,
}

#[derive(Serialize)]
pub struct AppStats {
    pub num_docs: u64,
}
