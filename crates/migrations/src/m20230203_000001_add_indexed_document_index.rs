use entities::sea_orm::Statement;
use sea_orm_migration::prelude::*;

use sea_orm_migration::sea_orm::ConnectionTrait;
pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20230203_000001_add_indexed_document_index.rs"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "CREATE INDEX IF NOT EXISTS `idx-indexed_document-doc_id` ON `indexed_document` (`doc_id`);".to_string(),
            ))
            .await?;

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
