use crate::sea_orm::Statement;
use entities::models::connection;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20221107_000001_add_account_col"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove old connections.
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "DELETE FROM connections".to_string(),
            ))
            .await?;

        // Add account column
        manager
            .alter_table(
                Table::alter()
                    .table(connection::Entity)
                    .add_column(ColumnDef::new(Alias::new("account")).string().not_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
