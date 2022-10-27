use crate::sea_orm::Statement;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;
pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20221024_000001_connection_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add connection table
        let new_table = r#"
            CREATE TABLE IF NOT EXISTS "connections" (
                "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                "name" text NOT NULL UNIQUE,
                "access_token" text NOT NULL,
                "refresh_token" text NOT NULL,
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
