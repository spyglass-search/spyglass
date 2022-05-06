use sea_orm::{ConnectionTrait, DatabaseConnection, Schema};

use crate::models::{
    crawl_queue, create_connection, fetch_history, indexed_document, link, resource_rule,
};

#[allow(dead_code)]
pub async fn setup_test_db() -> DatabaseConnection {
    let db = create_connection(true).await.unwrap();
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

    Ok(())
}
