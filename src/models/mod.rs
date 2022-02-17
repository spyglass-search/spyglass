use sea_orm::{ConnectOptions, Database, DatabaseConnection};

pub mod crawl_queue;
pub mod fetch_history;
pub mod indexed_document;
pub mod resource_rule;

use crate::config::Config;

// TODO: Switch to sea-orm from raw SQL
pub async fn create_connection(
    config: &Config,
    is_test: bool,
) -> anyhow::Result<DatabaseConnection> {
    let db_uri: String = if is_test {
        "sqlite::memory:".to_string()
    } else {
        format!(
            "sqlite://{}?mode=rwc",
            config.data_dir.join("db.sqlite").to_str().unwrap()
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
    use crate::config::Config;
    use crate::models::create_connection;

    #[tokio::test]
    async fn test_create_connection() {
        let config = Config::new();
        let res = create_connection(&config, true).await;
        assert!(res.is_ok());
    }
}
