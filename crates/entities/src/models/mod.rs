use sea_orm::{ConnectOptions, Database, DatabaseConnection};

pub mod bootstrap_queue;
pub mod crawl_queue;
pub mod fetch_history;
pub mod indexed_document;
pub mod lens;
pub mod link;
pub mod resource_rule;

use shared::config::Config;

pub async fn create_connection(
    config: &Config,
    is_test: bool,
) -> anyhow::Result<DatabaseConnection> {
    let db_uri = if is_test {
        "sqlite::memory:".to_string()
    } else {
        format!(
            "sqlite://{}?mode=rwc",
            config.data_dir().join("db.sqlite").to_str().unwrap()
        )
    };

    // See https://www.sea-ql.org/SeaORM/docs/install-and-config/connection
    // for more connection options
    let mut opt = ConnectOptions::new(db_uri);
    opt.max_connections(1).sqlx_logging(false);

    Ok(Database::connect(opt).await?)
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
