pub use sea_orm_migration::prelude::*;

use entities::models::create_connection;
use shared::config::Config;

mod m20220505_000001_create_table;
mod m20220508_000001_lens_and_crawl_queue_update;
mod m20220522_000001_bootstrap_queue_table;
mod m20220718_000001_add_cols_to_lens;
mod m20220823_000001_migrate_search_schema;
mod m20220917_000001_add_col_to_queue;
mod m20221023_000001_connection_table;
mod m20221031_000001_add_error_column_to_crawl_queue;
mod m20221101_000001_add_open_url_col;
mod utils;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220505_000001_create_table::Migration),
            Box::new(m20220508_000001_lens_and_crawl_queue_update::Migration),
            Box::new(m20220522_000001_bootstrap_queue_table::Migration),
            Box::new(m20220718_000001_add_cols_to_lens::Migration),
            Box::new(m20220823_000001_migrate_search_schema::Migration),
            Box::new(m20220917_000001_add_col_to_queue::Migration),
            Box::new(m20221023_000001_connection_table::Migration),
            Box::new(m20221031_000001_add_error_column_to_crawl_queue::Migration),
            Box::new(m20221101_000001_add_open_url_col::Migration),
        ]
    }
}

impl Migrator {
    pub async fn run_migrations() -> Result<(), DbErr> {
        let config = Config::new();

        let db = create_connection(&config, false)
            .await
            .expect("Unable to connect to db");

        match Migrator::up(&db, None).await {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                // This is ok, just the migrator being funky
                if !msg.contains("been applied but its file is missing") {
                    return Err(e);
                }

                Ok(())
            }
        }
    }
}
