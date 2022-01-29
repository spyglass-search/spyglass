use rocket::Config;

#[get("/test")]
pub fn test_route() -> &'static str {
    "Hello world"
}

pub async fn init_rocket() -> rocket::Shutdown {
    let config = Config {
        port: 7777,
        ..Config::debug_default()
    };

    let rocket = rocket::custom(&config)
        .mount("/api", routes![test_route])
        .ignite()
        .await
        .unwrap();

    let shutdown_handle = rocket.shutdown();
    tokio::spawn(rocket.launch());
    shutdown_handle
}
