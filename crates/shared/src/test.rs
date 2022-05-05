use sea_orm::DatabaseConnection;

use crate::models::{create_connection, setup_schema};

#[allow(dead_code)]
pub async fn setup_test_db() -> DatabaseConnection {
    let db = create_connection(true).await.unwrap();
    setup_schema(&db).await.expect("Unable to create tables");

    db
}
