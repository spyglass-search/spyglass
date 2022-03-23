use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Schema};

pub mod crawl_queue;
pub mod fetch_history;
pub mod indexed_document;
pub mod resource_rule;

use crate::config::Config;

pub async fn setup_schema(db: &DatabaseConnection) -> anyhow::Result<(), sea_orm::DbErr> {
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

    Ok(())
}

pub async fn create_connection(is_test: bool) -> anyhow::Result<DatabaseConnection> {
    let db_uri: String = if is_test {
        "sqlite::memory:".to_string()
    } else {
        format!(
            "sqlite://{}?mode=rwc",
            Config::data_dir().join("db.sqlite").to_str().unwrap()
        )
    };

    // See https://www.sea-ql.org/SeaORM/docs/install-and-config/connection
    // for more connection options
    let mut opt = ConnectOptions::new(db_uri);
    opt.max_connections(5).sqlx_logging(false);

    Ok(Database::connect(opt).await?)
}

#[cfg(test)]
mod test {
    use crate::models::create_connection;

    #[tokio::test]
    async fn test_create_connection() {
        let res = create_connection(true).await;
        assert!(res.is_ok());
    }
}
