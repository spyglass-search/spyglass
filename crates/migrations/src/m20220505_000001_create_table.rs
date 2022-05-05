use sea_schema::migration::prelude::*;
use shared::sea_orm;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20220505_000001_create_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(Table::create().to_owned()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
