use crate::sea_orm::Statement;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20221107_000001_recreate_connection_table"
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
                "DROP TABLE connections".to_string(),
            ))
            .await?;

        // Add account column
        let new_table = r#"
            CREATE TABLE IF NOT EXISTS "connections" (
                "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                "api_id" text NOT NULL,
                "account" text NOT NULL,
                "access_token" text NOT NULL,
                "refresh_token" text,
                "scopes" text NOT NULL,
                "expires_in" integer,
                "granted_at" text NOT NULL,
                "created_at" text NOT NULL,
                "updated_at" text NOT NULL);"#;

        // Create lens table
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                new_table.to_owned().to_string(),
            ))
            .await?;

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
