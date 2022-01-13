use sqlx::sqlite::{Sqlite, SqlitePoolOptions};
use sqlx::Pool;

mod crawl_queue;
mod fetch_history;
mod resource_rule;
// Flatten out models to `models::*` namespace.
pub use crawl_queue::*;
pub use fetch_history::*;
pub use resource_rule::*;

use crate::config::Config;

pub type DbPool = Pool<Sqlite>;

pub async fn create_connection(config: &Config) -> anyhow::Result<DbPool> {
    let db_uri = format!(
        "sqlite://{}?mode=rwc",
        config.data_dir.join("db.sqlite").to_str().unwrap()
    );

    Ok(SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_uri)
        .await?)
}

#[cfg(test)]
mod test {
    use crate::config::Config;
    use crate::models::create_connection;

    #[tokio::test]
    async fn test_create_connection() {
        let config = Config::new();
        let res = create_connection(&config).await;
        assert!(res.is_ok());
    }
}
