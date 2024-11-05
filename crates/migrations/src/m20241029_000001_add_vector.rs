use sea_orm_migration::prelude::*;

use entities::sea_orm::{ConnectionTrait, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let _ = manager
            .get_connection()
            .query_all(Statement::from_string(
                manager.get_database_backend(),
                "create virtual table vec_documents using vec0(
                    id integer primary key,
                    embedding float[768]
                );"
                .to_owned(),
            ))
            .await?;

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
