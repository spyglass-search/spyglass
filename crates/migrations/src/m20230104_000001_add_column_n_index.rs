use entities::models::{indexed_document, lens};
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20230104_000001_add_column_n_index"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add lens cache column if it doesn't exist
        if let Ok(has_col) = manager.has_column("lens", "last_cache_update").await {
            if has_col == false {
                manager
                    .alter_table(
                        Table::alter()
                            .table(lens::Entity)
                            .add_column(ColumnDef::new(Alias::new("last_cache_update")).date_time())
                            .to_owned(),
                    )
                    .await?;
            }
        }

        // Add unique index for url column, note altering the column
        // to make it unique is not allowed in sqlite, creating
        // a unique index is an alternative
        let rslt = manager
            .create_index(
                Index::create()
                    .name("idx-indexed_document-url")
                    .unique()
                    .table(indexed_document::Entity)
                    .col(indexed_document::Column::Url)
                    .to_owned(),
            )
            .await;

        // No need to fail, index may already exist
        if let Err(err) = rslt {
            log::error!("{:?}", err);
        }

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
