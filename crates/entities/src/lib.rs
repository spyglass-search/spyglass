use std::collections::HashMap;
pub mod models;
pub mod schema;
pub mod test;

pub use sea_orm;
use sea_orm::{DatabaseConnection, DbBackend, DbErr, FromQueryResult, Statement};

#[derive(Debug, FromQueryResult)]
pub struct CountByStatus {
    count: i64,
    name: String,
    status: String,
}

#[derive(Debug)]
pub struct LibraryStats {
    pub lens_name: String,
    pub crawled: i64,
    pub enqueued: i64,
    pub indexed: i64,
}

impl LibraryStats {
    pub fn new(name: &str) -> Self {
        LibraryStats {
            lens_name: name.to_owned(),
            crawled: 0,
            enqueued: 0,
            indexed: 0,
        }
    }

    pub fn total_docs(&self) -> i64 {
        if self.enqueued == 0 {
            self.indexed
        } else {
            self.crawled + self.enqueued
        }
    }

    pub fn percent_done(&self) -> i64 {
        self.crawled * 100 / (self.crawled + self.enqueued)
    }

    pub fn status_string(&self) -> String {
        format!("Crawling {} of {}", self.enqueued, self.total_docs())
    }
}

pub async fn get_library_stats(
    db: DatabaseConnection,
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
            GROUP BY tags.value, status
            UNION
            SELECT
                count(*) as "count", tags.value as "name", "Indexed" as status
            FROM indexed_document
            LEFT JOIN document_tag on indexed_document.id = document_tag.indexed_document_id
            LEFT JOIN tags on tags.id = document_tag.tag_id
            WHERE tags.label = "lens"
            group by tags.value, status;
        "#
        .to_string(),
    ))
    .all(&db)
    .await?;

    let mut stats: HashMap<String, LibraryStats> = HashMap::new();
    for count in counts {
        let entry = stats
            .entry(count.name.clone())
            .or_insert_with(|| LibraryStats::new(&count.name));
        match count.status.as_str() {
            "Completed" | "Failed" => {
                entry.crawled += count.count;
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
