use std::fs;
use std::path::PathBuf;

use sea_orm::{ConnectionTrait, DatabaseConnection, Schema};

use crate::config::Config;
use crate::models::{
    crawl_queue, create_connection, fetch_history, indexed_document, resource_rule,
};
use crate::search::{IndexPath, Searcher};

pub struct AppState {
    pub db: DatabaseConnection,
    pub config: Config,
    pub index: Searcher,
}

impl AppState {
    /// Initialize db tables
    pub async fn init_db(&self) {
        let builder = self.db.get_database_backend();
        let schema = Schema::new(builder);

        let mut create = builder.build(
            schema
                .create_table_from_entity(indexed_document::Entity)
                .if_not_exists(),
        );
        self.db.execute(create).await.unwrap();

        create = builder.build(
            schema
                .create_table_from_entity(crawl_queue::Entity)
                .if_not_exists(),
        );
        self.db.execute(create).await.unwrap();

        create = builder.build(
            schema
                .create_table_from_entity(fetch_history::Entity)
                .if_not_exists(),
        );
        self.db.execute(create).await.unwrap();

        create = builder.build(
            schema
                .create_table_from_entity(resource_rule::Entity)
                .if_not_exists(),
        );
        self.db.execute(create).await.unwrap();
    }

    pub fn crawl_dir() -> PathBuf {
        Config::data_dir().join("crawls")
    }

    pub fn index_dir() -> PathBuf {
        Config::data_dir().join("index")
    }

    pub async fn init_data_folders(&self) {
        fs::create_dir_all(AppState::crawl_dir()).expect("Unable to create crawl folder");
        fs::create_dir_all(AppState::index_dir()).expect("Unable to create index folder");
    }

    pub async fn new() -> Self {
        let config = Config::new();
        log::info!("config: {:?}", config);

        let db = create_connection(&config, false)
            .await
            .expect("Unable to connect to database");

        let index = Searcher::with_index(&IndexPath::LocalPath(Self::index_dir()));

        let app = AppState { db, config, index };
        app.init_db().await;
        app.init_data_folders().await;

        app
    }
}
