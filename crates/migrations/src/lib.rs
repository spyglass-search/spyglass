pub use sea_schema::migration::prelude::*;

mod m20220505_000001_create_table;
mod m20220508_000001_lens_and_crawl_queue_update;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220505_000001_create_table::Migration),
            Box::new(m20220508_000001_lens_and_crawl_queue_update::Migration),
        ]
    }
}
