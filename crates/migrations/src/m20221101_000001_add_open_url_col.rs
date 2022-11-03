use entities::models::indexed_document;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20221101_000001_add_open_url_col"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add is_enabled column
        manager
            .alter_table(
                Table::alter()
                    .table(indexed_document::Entity)
                    .add_column(ColumnDef::new(Alias::new("open_url")).string())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
