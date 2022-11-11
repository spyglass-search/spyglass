use crate::sea_orm::Statement;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20221109_000001_add_tags_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add tags table
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                r#"CREATE TABLE IF NOT EXISTS "tags" (
                    "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                    "label" text NOT NULL,
                    "value" text,
                    "created_at" text NOT NULL,
                    "updated_at" text NOT NULL
                );"#
                .to_string(),
            ))
            .await?;

        // Add through table
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                r#"CREATE TABLE IF NOT EXISTS "document_tag" (
                    "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                    "indexed_document_id" integer NOT NULL,
                    "tag_id" integer NOT NULL,
                    "created_at" text NOT NULL,
                    "updated_at" text NOT NULL,
                    FOREIGN KEY(indexed_document_id) REFERENCES indexed_document(id),
                    FOREIGN KEY(tag_id)              REFERENCES tags(id)
                );"#
                .to_string(),
            ))
            .await?;

        // Create index on (label, value). Should only every be one instance of a
        // tag with (label, value).
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "CREATE UNIQUE INDEX `idx-tag-label-value` ON `tags` (`label`, `value`);"
                    .to_string(),
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
