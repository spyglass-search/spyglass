use sea_orm_migration::{prelude::*, sea_orm::Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum EmbeddingQueue {
    #[iden = "embedding_queue"]
    Table,
    Id,
    DocumentId,
    Status,
    Errors,
    IndexedDocumentId,
    CreatedAt,
    UpdatedAt,
    Content,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EmbeddingQueue::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(EmbeddingQueue::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(EmbeddingQueue::DocumentId)
                            .string()
                            .unique_key()
                            .not_null(),
                    )
                    .col(ColumnDef::new(EmbeddingQueue::Content).string().null())
                    .col(ColumnDef::new(EmbeddingQueue::Status).string().not_null())
                    .col(ColumnDef::new(EmbeddingQueue::Errors).string().null())
                    .col(
                        ColumnDef::new(EmbeddingQueue::IndexedDocumentId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EmbeddingQueue::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EmbeddingQueue::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        if let Ok(true) = manager.has_table("embedding_queue").await {
            let status = r#"
                CREATE INDEX IF NOT EXISTS "embedding_queue_status" ON embedding_queue (status);"#;

            let indexed_document = r#"
                CREATE INDEX IF NOT EXISTS "idx-embedding_queue-indexed_document_id" ON embedding_queue (indexed_document_id);"#;

            for statement in &[status, indexed_document] {
                manager
                    .get_connection()
                    .execute(Statement::from_string(
                        manager.get_database_backend(),
                        statement.to_string(),
                    ))
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
