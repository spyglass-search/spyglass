use std::collections::HashSet;
use std::fmt;

use sea_orm::entity::prelude::*;
use sea_orm::{sea_query, DbBackend, FromQueryResult, QueryOrder, QuerySelect, Set, Statement};
use serde::Serialize;
use url::Url;

use super::indexed_document;
use crate::config::{Limit, UserSettings};

const MAX_RETRIES: u8 = 5;

#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(1))")]
pub enum CrawlStatus {
    #[sea_orm(string_value = "Queued")]
    Queued,
    #[sea_orm(string_value = "Processing")]
    Processing,
    #[sea_orm(string_value = "Completed")]
    Completed,
    #[sea_orm(string_value = "Failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "crawl_queue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Domain/host of the URL to be crawled
    pub domain: String,
    /// URL to crawl
    #[sea_orm(unique)]
    pub url: String,
    /// Task status.
    pub status: CrawlStatus,
    /// Number of retries for this task.
    #[sea_orm(default_value = 0)]
    pub num_retries: u8,
    /// Ignore crawl settings for this URL/domain and push to crawler.
    #[sea_orm(default_value = false)]
    pub force_crawl: bool,
    /// When this was first added to the crawl queue.
    pub created_at: DateTimeUtc,
    /// When this task was last updated.
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No RelationDef")
    }
}

impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            status: Set(CrawlStatus::Queued),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }

    // Triggered before insert / update
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        if !insert {
            self.updated_at = Set(chrono::Utc::now());
        }

        Ok(self)
    }
}

impl fmt::Display for CrawlStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CrawlStatus::Queued => write!(f, "Queued"),
            CrawlStatus::Processing => write!(f, "Processing"),
            CrawlStatus::Completed => write!(f, "Completed"),
            CrawlStatus::Failed => write!(f, "Failed"),
        }
    }
}

pub async fn reset_processing(db: &DatabaseConnection) {
    Entity::update_many()
        .col_expr(
            Column::Status,
            sea_query::Expr::value(sea_query::Value::String(Some(Box::new(
                CrawlStatus::Queued.to_string(),
            )))),
        )
        .filter(Column::Status.contains(&CrawlStatus::Processing.to_string()))
        .exec(db)
        .await
        .unwrap();
}

#[derive(FromQueryResult)]
struct CrawlQueueCount {
    count: i64,
}

pub async fn num_queued(db: &DatabaseConnection) -> anyhow::Result<u64, sea_orm::DbErr> {
    let res = Entity::find()
        .column_as(Column::Id.count(), "count")
        .filter(Column::Status.eq(CrawlStatus::Queued.to_string()))
        .into_model::<CrawlQueueCount>()
        .one(db)
        .await?;

    Ok(res.unwrap().count as u64)
}

/// Get the next url in the crawl queue
pub async fn dequeue(
    db: &DatabaseConnection,
    limit: Limit,
) -> anyhow::Result<Option<Model>, sea_orm::DbErr> {
    if let Limit::Infinite = limit {
        return Entity::find()
            .filter(Column::Status.eq(CrawlStatus::Queued.to_string()))
            .order_by_asc(Column::UpdatedAt)
            .one(db)
            .await;
    } else if let Limit::Finite(num_domains) = limit {
        let entity = Entity::find().from_raw_sql(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
                SELECT
                    cq.*
                FROM crawl_queue cq
                LEFT JOIN (
                    SELECT
                        domain,
                        count(*) as count
                    FROM indexed_document
                    GROUP BY domain
                ) as t on t.domain = cq.domain
                WHERE
                    COALESCE(t.count, 0) < ?
                    AND status = ?
                ORDER BY cq.updated_at ASC
            "#,
            vec![num_domains.into(), CrawlStatus::Queued.to_string().into()],
        ));

        return entity.one(db).await;
    }

    Ok(None)
}

/// Add url to the crawl queue
pub async fn enqueue(
    db: &DatabaseConnection,
    url: &str,
    settings: &UserSettings,
) -> anyhow::Result<(), sea_orm::DbErr> {
    let block_list: HashSet<String> = HashSet::from_iter(settings.block_list.iter().cloned());

    // Ignore invalid URLs
    let parsed = Url::parse(url);
    if parsed.is_err() {
        log::debug!("Url ignored: invalid URL - {}", url);
        return Ok(());
    }
    let parsed = parsed.unwrap();

    let domain = parsed.host_str();
    // Ignore URLs w/ no domain/host strings
    if domain.is_none() {
        log::debug!("Url ignored: invalid domain - {}", url);
        return Ok(());
    }

    // Ignore domains in blocklist
    let domain = domain.unwrap();
    if block_list.contains(&domain.to_string()) {
        log::debug!("Url ignored: blocked domain - {}", url);
        return Ok(());
    }

    let exists = Entity::find()
        .filter(Column::Url.eq(url.to_string()))
        .one(db)
        .await?;

    // ignore duplicate urls
    if exists.is_some() {
        log::debug!("Url ignored: duplicate crawl - {}", url);
        return Ok(());
    }

    // ignore already indexed docs
    let already_indexed = indexed_document::Entity::find()
        .filter(indexed_document::Column::Url.eq(url.to_string()))
        .one(db)
        .await?
        .is_some();

    if already_indexed {
        log::info!("Url ignored: already indexed - {}", url);
        return Ok(());
    }

    let new_task = ActiveModel {
        domain: Set(domain.to_string()),
        url: Set(url.to_owned()),
        ..Default::default()
    };
    new_task.insert(db).await?;
    Ok(())
}

pub async fn mark_done(
    db: &DatabaseConnection,
    id: i64,
    status: CrawlStatus,
) -> anyhow::Result<()> {
    let crawl = Entity::find_by_id(id).one(db).await?.unwrap();
    let mut updated: ActiveModel = crawl.clone().into();

    // Bump up number of retries if this failed
    if status == CrawlStatus::Failed && crawl.num_retries <= MAX_RETRIES {
        updated.num_retries = Set(crawl.num_retries + 1);
        // Queue again
        updated.status = Set(CrawlStatus::Queued);
    } else {
        updated.status = Set(status);
    }

    updated.update(db).await?;

    Ok(())
}

#[cfg(test)]
mod test {
    use sea_orm::prelude::*;
    use sea_orm::{ActiveModelTrait, Set};
    use url::Url;

    use crate::config::{Limit, UserSettings};
    use crate::models::{crawl_queue, indexed_document};
    use crate::test::setup_test_db;

    #[tokio::test]
    async fn test_insert() {
        let db = setup_test_db().await;

        let url = "oldschool.runescape.wiki/";
        let crawl = crawl_queue::ActiveModel {
            domain: Set("oldschool.runescape.wiki".to_string()),
            url: Set(url.to_owned()),
            ..Default::default()
        };
        crawl.insert(&db).await.expect("Unable to insert");

        let query = crawl_queue::Entity::find()
            .filter(crawl_queue::Column::Url.eq(url.to_string()))
            .one(&db)
            .await
            .expect("Unable to run query");

        assert!(query.is_some());

        let res = query.unwrap();
        assert_eq!(res.url, url);
    }

    #[tokio::test]
    async fn test_enqueue() {
        let settings = UserSettings::default();
        let db = setup_test_db().await;
        let url = "https://oldschool.runescape.wiki/";
        crawl_queue::enqueue(&db, url, &settings).await.unwrap();

        let crawl = crawl_queue::Entity::find()
            .filter(crawl_queue::Column::Url.eq(url.to_string()))
            .all(&db)
            .await
            .unwrap();

        assert_eq!(crawl.len(), 1);
    }

    #[tokio::test]
    async fn test_dequeue() {
        let settings = UserSettings::default();
        let db = setup_test_db().await;
        let url = "https://oldschool.runescape.wiki/";
        crawl_queue::enqueue(&db, url, &settings).await.unwrap();

        let queue = crawl_queue::dequeue(&db, Limit::Infinite).await.unwrap();
        assert!(queue.is_some());
        assert_eq!(queue.unwrap().url, url);
    }

    #[tokio::test]
    async fn test_dequeue_with_limit() {
        let settings = UserSettings::default();
        let db = setup_test_db().await;
        let url = "https://oldschool.runescape.wiki/";
        let parsed = Url::parse(&url).unwrap();

        crawl_queue::enqueue(&db, url, &settings).await.unwrap();
        let doc = indexed_document::ActiveModel {
            domain: Set(parsed.host_str().unwrap().to_string()),
            url: Set(url.to_string()),
            doc_id: Set("docid".to_string()),
            ..Default::default()
        };
        doc.save(&db).await.unwrap();
        let queue = crawl_queue::dequeue(&db, Limit::Finite(2)).await.unwrap();
        assert!(queue.is_some());

        let queue = crawl_queue::dequeue(&db, Limit::Finite(1)).await.unwrap();
        assert!(queue.is_none());
    }
}
