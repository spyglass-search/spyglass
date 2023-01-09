use entities::{models::lens, sea_orm::Statement};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};
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
            if !has_col {
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

        let val = manager
            .get_connection()
            .query_all(Statement::from_string(
                manager.get_database_backend(),
                r#"with url_counts as (
                select count(*) as cnt,url 
                from indexed_document 
                group by url
            ) 
            select * from url_counts where cnt > 1"#
                    .into(),
            ))
            .await?;

        // Running the duplicate delete is extremely slow without an index so added a temp one
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "CREATE INDEX `tmp-idx-indexed_document-url` ON `indexed_document` (`url`);"
                    .to_string(),
            ))
            .await?;

        let txn = manager.get_connection().begin().await?;

        for row in val {
            let url_rslt = row.try_get::<String>("", "url");
            if let Ok(url) = url_rslt {
                // delete tags
                txn.execute(Statement::from_sql_and_values(
                    manager.get_database_backend(),
                    r#"with id_list as (
                        select id
                        from indexed_document where url = ? 
                        order by updated_at desc
                        ),
                     id_keep as (
                        select id
                        from indexed_document where url = ? 
                        order by updated_at desc limit 1
                        )
                    delete from document_tag where indexed_document_id in id_list and indexed_document_id not in id_keep"#, 
                    vec![url.clone().into()])).await?;

                // delete indexed documents
                txn.execute(Statement::from_sql_and_values(
                    manager.get_database_backend(),
                    r#"with id_list as (
                        select id
                        from indexed_document where url = ? 
                        order by updated_at desc
                        ),
                     id_keep as (
                        select id
                        from indexed_document where url = ? 
                        order by updated_at desc limit 1
                        )
                    delete from indexed_document where id in id_list and id not in id_keep"#,
                    vec![url.into()],
                ))
                .await?;
            }
        }

        txn.commit().await?;

        // removing temp index
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "DROP INDEX `tmp-idx-indexed_document-url`;".to_string(),
            ))
            .await?;

        // Add unique index for url column, note altering the column
        // to make it unique is not allowed in sqlite, creating
        // a unique index is an alternative
        let rslt = manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "CREATE UNIQUE INDEX `idx-indexed_document-url` ON `indexed_document` (`url`);"
                    .to_string(),
            ))
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
