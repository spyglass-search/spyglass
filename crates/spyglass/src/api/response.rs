use serde::Serialize;
use shared::models::crawl_queue;

#[derive(Serialize)]
pub struct ListQueue {
    pub queue: Vec<crawl_queue::Model>,
}
