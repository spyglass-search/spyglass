use sea_orm_migration::{prelude::*, sea_orm::Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum VecToIndexed {
    #[iden = "vec_to_indexed"]
    Table,
    Id,
    IndexedId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum IndexedDocument {
    #[iden = "indexed_document"]
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(VecToIndexed::Table)
                    .if_not_exists()
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-vec_to_indexed-indexed_document")
                            .from(VecToIndexed::Table, VecToIndexed::IndexedId)
                            .to(IndexedDocument::Table, IndexedDocument::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .col(
                        ColumnDef::new(VecToIndexed::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(VecToIndexed::IndexedId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VecToIndexed::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VecToIndexed::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
