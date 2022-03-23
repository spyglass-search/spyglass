mod response;
mod route;

use sea_orm::DatabaseConnection;
use tantivy::{Index, IndexReader};

use crate::config::Config;

pub async fn start_api(
    db: DatabaseConnection,
    config: &Config,
    index: &Index,
    reader: &IndexReader,
) -> rocket::Shutdown {
    let api_config = rocket::Config {
        port: 7777,
        ..rocket::Config::debug_default()
    };

    let rocket = rocket::custom(&api_config)
        .manage::<Config>(config.clone())
        .manage::<DatabaseConnection>(db)
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
