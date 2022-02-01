mod response;
mod route;

use rocket::Config;
use tantivy::IndexReader;

use crate::models::DbPool;

pub async fn init_rocket(pool: &DbPool, search: &IndexReader) -> rocket::Shutdown {
    let config = Config {
        port: 7777,
        ..Config::debug_default()
    };

    let rocket = rocket::custom(&config)
        .manage::<DbPool>(pool.clone())
        .manage::<IndexReader>(search.clone())
        .mount("/api", routes![route::list_queue, route::app_stats])
        .ignite()
        .await
        .unwrap();

    let shutdown_handle = rocket.shutdown();
    tokio::spawn(rocket.launch());
    shutdown_handle
}
