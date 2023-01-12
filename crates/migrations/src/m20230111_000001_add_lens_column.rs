use entities::models::lens;
use sea_orm_migration::prelude::*;
pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20230111_000001_add_lens_column"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add remote url column, column is used to hold the url that the
        // lens was installed from
        if let Ok(has_col) = manager.has_column("lens", "remote_url").await {
            if !has_col {
                manager
                    .alter_table(
                        Table::alter()
                            .table(lens::Entity)
                            .add_column(ColumnDef::new(Alias::new("remote_url")).string())
                            .to_owned(),
                    )
                    .await?;
            }
        }

        // Adds a hash column used to identify if the lens has changed or not since
        // last load
        if let Ok(has_col) = manager.has_column("lens", "hash").await {
            if !has_col {
                manager
                    .alter_table(
                        Table::alter()
                            .table(lens::Entity)
                            .add_column(ColumnDef::new(Alias::new("hash")).string())
                            .to_owned(),
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
