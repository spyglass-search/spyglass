use crate::models::CrawlQueue;
use rocket::serde::Serialize;

#[derive(Serialize)]
pub struct ListQueue {
    pub queue: Vec<CrawlQueue>,
}

#[derive(Serialize)]
pub struct AppStats {
    pub num_docs: u64,
}
