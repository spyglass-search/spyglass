use sea_orm::{ConnectOptions, Database, DatabaseConnection};

pub mod bootstrap_queue;
pub mod connection;
pub mod crawl_queue;
pub mod crawl_tag;
pub mod document_tag;
pub mod fetch_history;
pub mod indexed_document;
pub mod lens;
pub mod link;
pub mod processed_files;
pub mod resource_rule;
pub mod schema;
pub mod tag;

use shared::config::Config;

/// Creates a connection based on the passed in
/// configuration
pub async fn create_connection(
    config: &Config,
    is_test: bool,
) -> anyhow::Result<DatabaseConnection> {
    let db_uri = if is_test {
        "sqlite::memory:".to_string()
    } else {
        format!(
            "sqlite://{}?mode=rwc",
            config
                .data_dir()
                .join("db.sqlite")
                .to_str()
                .expect("Unable to create db")
        )
    };

    create_connection_by_uri(&db_uri).await
}

/// Creates a connection based on the database uri
pub async fn create_connection_by_uri(db_uri: &str) -> anyhow::Result<DatabaseConnection> {
    // See https://www.sea-ql.org/SeaORM/docs/install-and-config/connection
    // for more connection options
    let mut opt = ConnectOptions::new(db_uri.to_owned());
    opt.max_connections(10)
        .min_connections(2)
        .sqlx_logging(false);

    Ok(Database::connect(opt).await?)
}

// Helper method used to copy all tables from one database to another.
// Note that the destination database will have all content deleted.
pub async fn copy_all_tables(
    from: &DatabaseConnection,
    to: &DatabaseConnection,
) -> anyhow::Result<(), sea_orm::DbErr> {
    bootstrap_queue::copy_table(from, to).await?;
    connection::copy_table(from, to).await?;
    crawl_queue::copy_table(from, to).await?;
    fetch_history::copy_table(from, to).await?;
    indexed_document::copy_table(from, to).await?;
    lens::copy_table(from, to).await?;
    link::copy_table(from, to).await?;
    processed_files::copy_table(from, to).await?;
    resource_rule::copy_table(from, to).await?;
    tag::copy_table(from, to).await?;
    document_tag::copy_table(from, to).await?;
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::models::create_connection;
    use shared::config::Config;

    #[tokio::test]
    async fn test_create_connection() {
        let config = Config::default();
        let res = create_connection(&config, true).await;
        assert!(res.is_ok());
    }
}
