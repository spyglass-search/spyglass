use rusqlite::Result;
use simple_logger::SimpleLogger;

use carto::Carto;

mod config;
mod importer;
use crate::config::Config;
use crate::importer::FirefoxImporter;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging system
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .with_utc_timestamps()
        .init()
        .unwrap();

    let config = Config::new();
    log::info!("config: {:?}", config);

    // Initialize crawler
    let carto = Carto::init(&config.data_dir);

    // Import data from Firefox
    // TODO: Ask user what browser/profiles to import on first startup.
    let importer = FirefoxImporter::new(&config);
    let _ = importer.import(&carto);

    carto.run().await;

    Ok(())
}
