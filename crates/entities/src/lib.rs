use std::collections::HashMap;
pub mod models;
pub mod schema;
pub mod test;

pub use sea_orm;
use sea_orm::{DatabaseConnection, DbBackend, DbErr, FromQueryResult, Statement};
use shared::response::LibraryStats;

pub const BATCH_SIZE: usize = 3000;

#[derive(Debug, FromQueryResult)]
pub struct CountByStatus {
    count: i32,
    name: String,
    status: String,
}

pub async fn get_library_stats(
    db: &DatabaseConnection,
) -> Result<HashMap<String, LibraryStats>, DbErr> {
    let counts = CountByStatus::find_by_statement(Statement::from_string(
        DbBackend::Sqlite,
        r#"
            SELECT
                count(*) as "count", tags.value as "name", status
            FROM crawl_queue
            LEFT JOIN crawl_tag on crawl_queue.id = crawl_tag.crawl_queue_id
            LEFT JOIN tags on tags.id = crawl_tag.tag_id
            WHERE tags.label = "lens"
            GROUP BY lower(tags.value), lower(status)
            UNION
            SELECT
                count(*) as "count", tags.value as "name", "Indexed" as status
            FROM indexed_document
            LEFT JOIN document_tag on indexed_document.id = document_tag.indexed_document_id
            LEFT JOIN tags on tags.id = document_tag.tag_id
            WHERE tags.label = "lens"
            group by lower(tags.value), lower(status);
        "#
        .to_string(),
    ))
    .all(db)
    .await?;

    let mut stats: HashMap<String, LibraryStats> = HashMap::new();
    for count in counts {
        let entry = stats
            .entry(count.name.clone())
            .or_insert_with(|| LibraryStats::new(&count.name));
        match count.status.as_str() {
            "Completed" => {
                entry.crawled += count.count;
            }
            "Failed" => {
                entry.crawled += count.count;
                entry.failed += count.count;
            }
            "Queued" | "Processing" => {
                entry.enqueued += count.count;
            }
            "Indexed" => {
                entry.indexed += count.count;
            }
            _ => {}
        }
    }

    Ok(stats)
}
