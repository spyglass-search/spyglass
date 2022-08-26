pub use sea_orm_migration::prelude::*;

mod m20220505_000001_create_table;
mod m20220508_000001_lens_and_crawl_queue_update;
mod m20220522_000001_bootstrap_queue_table;
mod m20220718_000001_add_cols_to_lens;
mod m20220823_000001_migrate_search_schema;

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
        ]
    }
}
