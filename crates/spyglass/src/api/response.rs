use libspyglass::models::crawl_queue;
use rocket::serde::Serialize;

#[derive(Serialize)]
pub struct ListQueue {
    pub queue: Vec<crawl_queue::Model>,
}
