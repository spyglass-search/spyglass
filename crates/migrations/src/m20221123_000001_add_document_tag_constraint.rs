use crate::sea_orm::Statement;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20221123_000001_add_document_tag_constraint"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Clear out tag connections (if any).
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "DELETE FROM document_tag".to_string(),
            ))
            .await?;

        // Create index on (indexed_document_id, tag_id). Should only every be one instance of a
        // tag with (label, value) attached to a document.
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "CREATE UNIQUE INDEX `idx-document-tag-doc-id-tag-id` ON `document_tag` (`indexed_document_id`, `tag_id`);"
                    .to_string(),
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
