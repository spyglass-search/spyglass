use rusqlite::{params, Connection, OpenFlags, Result};
use simple_logger::SimpleLogger;
use url::Url;

use carto::{models::Place, Carto};

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
    let carto = Carto::init();

    // Detect profiles, do we need to import data?
    let importer = FirefoxImporter::new(&config);
    if let Ok(db_path) = importer.import() {
        let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        println!("Connected to db...");

        let mut stmt = conn.prepare("SELECT id, url FROM moz_places where hidden = 0 LIMIT 1")?;
        let place_iter = stmt.query_map(params![], |row| {
            let url_str: String = row.get(1)?;
            let url = Url::parse(&url_str).unwrap();
            Ok(Place {
                id: row.get(0)?,
                url,
            })
        })?;

        for place in place_iter {
            let place = place.unwrap();
            carto.fetch(&place).await.expect("unable to fetch");
        }
    }

    Ok(())
}
