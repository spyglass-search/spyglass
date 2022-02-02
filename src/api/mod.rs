mod response;
mod route;

use rocket::Config;
use tantivy::{Index, IndexReader};

use crate::models::DbPool;

pub async fn start_api(pool: &DbPool, index: &Index, reader: &IndexReader) -> rocket::Shutdown {
    let config = Config {
        port: 7777,
        ..Config::debug_default()
    };

    let rocket = rocket::custom(&config)
        .manage::<DbPool>(pool.clone())
        .manage::<Index>(index.clone())
        .manage::<IndexReader>(reader.clone())
        .mount(
            "/api",
            routes![
                // queue routes
                route::add_queue,
                route::list_queue,
                // search
                route::search,
                // app stats
                route::app_stats
            ],
        )
        .ignite()
        .await
        .unwrap();

    let shutdown_handle = rocket.shutdown();
    tokio::spawn(rocket.launch());
    shutdown_handle
}
