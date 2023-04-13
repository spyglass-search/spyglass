use crate::sea_orm::Statement;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DbBackend};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20221109_000001_add_tags_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let tables = if manager.get_database_backend() == DbBackend::Sqlite {
            let tags = r#"CREATE TABLE IF NOT EXISTS "tags" (
                "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                "label" text NOT NULL,
                "value" text,
                "created_at" text NOT NULL,
                "updated_at" text NOT NULL
            );"#;
            let doc_tag = r#"CREATE TABLE IF NOT EXISTS "document_tag" (
                "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                "indexed_document_id" integer NOT NULL,
                "tag_id" integer NOT NULL,
                "created_at" text NOT NULL,
                "updated_at" text NOT NULL,
                FOREIGN KEY(indexed_document_id) REFERENCES indexed_document(id),
                FOREIGN KEY(tag_id)              REFERENCES tags(id)
            );"#;
            Some([tags, doc_tag])
        } else if manager.get_database_backend() == DbBackend::Postgres {
            let tags = r#"CREATE TABLE IF NOT EXISTS "tags" (
                "id" BIGSERIAL PRIMARY KEY,
                "label" text NOT NULL,
                "value" text,
                "created_at" TIMESTAMPTZ NOT NULL,
                "updated_at" TIMESTAMPTZ NOT NULL
            );"#;
            let doc_tag = r#"CREATE TABLE IF NOT EXISTS "document_tag" (
                "id" BIGSERIAL PRIMARY KEY,
                "indexed_document_id" integer NOT NULL,
                "tag_id" integer NOT NULL,
                "created_at" TIMESTAMPTZ NOT NULL,
                "updated_at" TIMESTAMPTZ NOT NULL,
                CONSTRAINT fk_tag_id
                    FOREIGN KEY(tag_id)
                     REFERENCES tags(id),
                CONSTRAINT fk_indexed_document_id
                            FOREIGN KEY(indexed_document_id)
                            REFERENCES indexed_document(id)
            );"#;
            Some([tags, doc_tag])
        } else {
            None
        };

        if let Some(tables) = tables {
            for tbl in tables {
                manager
                    .get_connection()
                    .execute(Statement::from_string(
                        manager.get_database_backend(),
                        tbl.to_string(),
                    ))
                    .await?;
            }
        }

        // Create index on (label, value). Should only every be one instance of a
        // tag with (label, value).
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "CREATE UNIQUE INDEX IF NOT EXISTS \"idx-tag-label-value\" ON \"tags\" (\"label\", \"value\");"
                    .to_string(),
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
