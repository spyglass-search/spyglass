use entities::sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20230126_000001_create_file_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let processed_files = r#"
            CREATE TABLE IF NOT EXISTS "processed_files" (
                "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                "file_path" text NOT NULL UNIQUE,
                "created_at" text NOT NULL,
                "last_modified" text NOT NULL);"#;

        for sql in &[processed_files] {
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    sql.to_owned().to_string(),
                ))
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
