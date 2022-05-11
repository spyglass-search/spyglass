use entities::sea_orm::{ConnectionTrait, Statement};
use sea_schema::migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20220505_000001_create_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let crawl_queue = r#"
            CREATE TABLE IF NOT EXISTS "crawl_queue" (
                "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                "domain" text NOT NULL,
                "url" text NOT NULL UNIQUE,
                "status" text NOT NULL,
                "num_retries" integer NOT NULL DEFAULT 0,
                "force_crawl" integer NOT NULL DEFAULT FALSE,
                "created_at" text NOT NULL,
                "updated_at" text NOT NULL);"#;

        let fetch_history = r#"
            CREATE TABLE IF NOT EXISTS "fetch_history" (
                "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                "protocol" text NOT NULL,
                "domain" text NOT NULL,
                "path" text NOT NULL,
                "hash" text,
                "status" integer NOT NULL,
                "no_index" integer NOT NULL DEFAULT FALSE,
                "created_at" text NOT NULL,
                "updated_at" text NOT NULL );"#;

        let indexed_document = r#"
            CREATE TABLE IF NOT EXISTS "indexed_document" (
                "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                "domain" text NOT NULL,
                "url" text NOT NULL,
                "doc_id" text NOT NULL,
                "created_at" text NOT NULL,
                "updated_at" text NOT NULL );
        "#;

        let resource_rules = r#"
            CREATE TABLE IF NOT EXISTS "resource_rules" (
                "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                "domain" text NOT NULL,
                "rule" text NOT NULL,
                "no_index" integer NOT NULL,
                "allow_crawl" integer NOT NULL,
                "created_at" text NOT NULL,
                "updated_at" text NOT NULL );"#;

        let link = r#"
            CREATE TABLE IF NOT EXISTS "link" (
                "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                "src_domain" text NOT NULL,
                "src_url" text NOT NULL,
                "dst_domain" text NOT NULL,
                "dst_url" text NOT NULL );"#;

        for sql in &[
            crawl_queue,
            fetch_history,
            indexed_document,
            resource_rules,
            link,
        ] {
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    sql.to_owned().to_string(),
                ))
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
