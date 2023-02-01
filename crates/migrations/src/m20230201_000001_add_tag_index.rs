use entities::sea_orm::Statement;
use sea_orm_migration::prelude::*;

use sea_orm_migration::sea_orm::ConnectionTrait;
pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20230201_000001_add_tag_index.rs"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "CREATE INDEX `idx-tag-value` ON `tags` (`value`);".to_string(),
            ))
            .await?;

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
