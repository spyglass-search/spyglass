use entities::models::lens;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20220718_000001_add_cols_to_lens"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add is_enabled column
        manager
            .alter_table(
                Table::alter()
                    .table(lens::Entity)
                    .add_column(
                        ColumnDef::new(Alias::new("is_enabled"))
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .to_owned(),
            )
            .await?;

        // Add lens_type column
        manager
            .alter_table(
                Table::alter()
                    .table(lens::Entity)
                    .add_column(
                        ColumnDef::new(Alias::new("lens_type"))
                            .string()
                            .not_null()
                            .default(lens::LensType::Simple),
                    )
                    .to_owned(),
            )
            .await?;

        // Add trigger column
        manager
            .alter_table(
                Table::alter()
                    .table(lens::Entity)
                    .add_column(ColumnDef::new(Alias::new("trigger")).string())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
