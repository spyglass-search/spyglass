use sea_orm::{sea_query::Index, ConnectionTrait, DatabaseConnection, Schema};
use shared::config::Config;

use crate::models::{
    bootstrap_queue, crawl_queue, crawl_tag, create_connection, document_tag, fetch_history,
    indexed_document, lens, link, resource_rule, tag,
};

#[allow(dead_code)]
pub async fn setup_test_db() -> DatabaseConnection {
    let config = Config::default();
    let db = create_connection(&config, true).await.unwrap();
    setup_schema(&db).await.expect("Unable to create tables");

    db
}

#[allow(dead_code)]
/// Only used during testing
async fn setup_schema(db: &DatabaseConnection) -> anyhow::Result<(), sea_orm::DbErr> {
    let builder = db.get_database_backend();
    let schema = Schema::new(builder);

    db.execute(
        builder.build(
            schema
                .create_table_from_entity(crawl_queue::Entity)
                .if_not_exists(),
        ),
    )
    .await?;
    db.execute(
        builder.build(
            schema
                .create_table_from_entity(fetch_history::Entity)
                .if_not_exists(),
        ),
    )
    .await?;
    db.execute(
        builder.build(
            schema
                .create_table_from_entity(indexed_document::Entity)
                .if_not_exists(),
        ),
    )
    .await?;
    db.execute(
        builder.build(
            schema
                .create_table_from_entity(resource_rule::Entity)
                .if_not_exists(),
        ),
    )
    .await?;
    db.execute(
        builder.build(
            schema
                .create_table_from_entity(link::Entity)
                .if_not_exists(),
        ),
    )
    .await?;

    db.execute(
        builder.build(
            schema
                .create_table_from_entity(lens::Entity)
                .if_not_exists(),
        ),
    )
    .await?;

    db.execute(
        builder.build(
            schema
                .create_table_from_entity(bootstrap_queue::Entity)
                .if_not_exists(),
        ),
    )
    .await?;

    db.execute(builder.build(schema.create_table_from_entity(tag::Entity).if_not_exists()))
        .await?;

    db.execute(
        builder.build(
            schema
                .create_table_from_entity(document_tag::Entity)
                .if_not_exists(),
        ),
    )
    .await?;

    db.execute(
        builder.build(
            schema
                .create_table_from_entity(crawl_tag::Entity)
                .if_not_exists(),
        ),
    )
    .await?;

    db.execute(
        builder.build(
            &Index::create()
                .unique()
                .name("idx-tags-label-value")
                .table(tag::Entity)
                .col(tag::Column::Label)
                .col(tag::Column::Value)
                .to_owned(),
        ),
    )
    .await?;

    db.execute(
        builder.build(
            &Index::create()
                .unique()
                .name("idx-document-tag-doc-id-tag-id")
                .table(document_tag::Entity)
                .col(document_tag::Column::IndexedDocumentId)
                .col(document_tag::Column::TagId)
                .to_owned(),
        ),
    )
    .await?;

    db.execute(
        builder.build(
            &Index::create()
                .unique()
                .name("idx-crawl-tag-doc-id-tag-id")
                .table(crawl_tag::Entity)
                .col(crawl_tag::Column::CrawlQueueId)
                .col(crawl_tag::Column::TagId)
                .to_owned(),
        ),
    )
    .await?;

    Ok(())
}
