use entities::{
    models::crawl_queue,
    sea_orm::{ConnectionTrait, Statement},
};
use sea_orm::DbBackend;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20220508_000001_lens_and_crawl_queue_update"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let lens_table = if manager.get_database_backend() == DbBackend::Sqlite {
            Some(
                r#"
                CREATE TABLE IF NOT EXISTS "lens" (
                    "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                    "name" text NOT NULL UNIQUE,
                    "author" text NOT NULL,
                    "description" text NOT NULL,
                    "version" text NOT NULL);"#,
            )
        } else if manager.get_database_backend() == DbBackend::Postgres {
            Some(
                r#"
                CREATE TABLE IF NOT EXISTS "lens" (
                    "id" BIGSERIAL PRIMARY KEY,
                    "name" text NOT NULL UNIQUE,
                    "author" text NOT NULL,
                    "description" text NOT NULL,
                    "version" text NOT NULL);"#,
            )
        } else {
            None
        };

        if let Some(lens_table) = lens_table {
            // Create lens table
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    lens_table.to_owned().to_string(),
                ))
                .await?;
        }

        // Add crawl_type column
        manager
            .alter_table(
                Table::alter()
                    .table(crawl_queue::Entity)
                    .add_column(
                        ColumnDef::new(Alias::new("crawl_type"))
                            .string()
                            .not_null()
                            .default(crawl_queue::CrawlType::Normal),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
