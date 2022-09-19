use entities::models::crawl_queue;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20220917_000001_add_col_to_queue"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add is_enabled column
        manager
            .alter_table(
                Table::alter()
                    .table(crawl_queue::Entity)
                    .add_column(ColumnDef::new(Alias::new("pipeline")).string())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
