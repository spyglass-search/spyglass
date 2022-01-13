use anyhow::Result;
use simple_logger::SimpleLogger;

mod config;
mod crawler;
mod importer;
mod models;

use crate::config::Config;
use crate::crawler::Carto;
use crate::importer::FirefoxImporter;
use crate::models::{create_connection, DbPool};

struct AppState {
    pub conn: DbPool,
    pub config: Config,
}

impl AppState {
    pub async fn new() -> Self {
        let config = Config::new();
        log::info!("config: {:?}", config);

        let conn = create_connection(&config)
            .await
            .expect("Unable to connect to database");
        AppState { conn, config }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging system
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .with_utc_timestamps()
        .init()
        .unwrap();

    let state = AppState::new().await;

    // Initialize crawler
    let carto = Carto::init(&state.config.data_dir, &state.conn).await;

    // Import data from Firefox
    // TODO: Ask user what browser/profiles to import on first startup.
    let importer = FirefoxImporter::new(&state.config);
    let _ = importer.import(&carto).await;

    carto.run().await;

    Ok(())
}
