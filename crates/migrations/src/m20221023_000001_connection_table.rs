use crate::sea_orm::Statement;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DbBackend};
pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20221024_000001_connection_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let new_table = if manager.get_database_backend() == DbBackend::Sqlite {
            Some(
                r#"
                CREATE TABLE IF NOT EXISTS "connections" (
                    "id" text NOT NULL PRIMARY KEY,
                    "access_token" text NOT NULL,
                    "refresh_token" text,
                    "scopes" text NOT NULL,
                    "expires_in" integer,
                    "granted_at" text NOT NULL,
                    "created_at" text NOT NULL,
                    "updated_at" text NOT NULL);"#,
            )
        } else if manager.get_database_backend() == DbBackend::Postgres {
            Some(
                r#"
                CREATE TABLE IF NOT EXISTS "connections" (
                    "id" text NOT NULL PRIMARY KEY,
                    "access_token" text NOT NULL,
                    "refresh_token" text,
                    "scopes" text NOT NULL,
                    "expires_in" integer,
                    "granted_at" TIMESTAMPTZ NOT NULL,
                    "created_at" TIMESTAMPTZ NOT NULL,
                    "updated_at" TIMESTAMPTZ NOT NULL);"#,
            )
        } else {
            None
        };

        if let Some(new_table) = new_table {
            // Create lens table
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    new_table.to_owned().to_string(),
                ))
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
