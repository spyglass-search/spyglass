mod param;
mod response;
mod route;

use crate::state::AppState;

pub async fn start_api(state: AppState) -> rocket::Shutdown {
    let api_config = rocket::Config {
        port: 7777,
        ..rocket::Config::debug_default()
    };

    let rocket = rocket::custom(&api_config)
        .manage::<AppState>(state.clone())
        .mount(
            "/api",
            routes![
                // queue routes
                route::add_queue,
                route::list_queue,
                // search
                route::search,
                // app stats
                route::app_stats,
                // Pause/unpause crawler
                route::update_app_status,
            ],
        )
        .ignite()
        .await
        .unwrap();

    let shutdown_handle = rocket.shutdown();
    tokio::spawn(rocket.launch());
    shutdown_handle
}
