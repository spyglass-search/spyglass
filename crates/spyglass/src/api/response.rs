use entities::models::crawl_queue;
use serde::Serialize;

#[derive(Serialize)]
pub struct ListQueue {
    pub queue: Vec<crawl_queue::Model>,
}
