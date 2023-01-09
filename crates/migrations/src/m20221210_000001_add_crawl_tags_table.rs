use entities::{
    models::{
        crawl_queue, crawl_tag,
        lens::{self, LensType},
        tag::{get_or_create, TagType},
    },
    sea_orm::{
        ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set,
        Statement, TransactionTrait,
    },
};
use sea_orm_migration::prelude::*;
use shared::config::{Config, LensConfig};
use std::time::Instant;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20221210_000001_add_crawl_tags_table"
    }
}

async fn add_tags_for_url<C>(tx: &C, name: &str, url: &str) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    // Ignore http/https when querying database
    let url = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    // Update existing tasks
    let start_time = Instant::now();
    let existing_tasks = crawl_queue::Entity::find()
        .filter(crawl_queue::Column::Url.contains(url))
        .all(tx)
        .await?;

    let tag = get_or_create(tx, TagType::Lens, name).await?;
    // create connections for each tag
    let task_tags = existing_tasks
        .iter()
        .map(|task| crawl_tag::ActiveModel {
            crawl_queue_id: Set(task.id),
            tag_id: Set(tag.id),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..Default::default()
        })
        .collect::<Vec<crawl_tag::ActiveModel>>();

    // Insert connections, ignoring duplicates
    for chunk in task_tags.chunks(5000) {
        crawl_tag::Entity::insert_many(chunk.to_vec())
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns(vec![
                    crawl_tag::Column::CrawlQueueId,
                    crawl_tag::Column::TagId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(tx)
            .await?;
    }

    let count = existing_tasks.len();
    let time_taken = Instant::now() - start_time;
    log::info!(
        "{}: tagged {} tasks in {}ms",
        name,
        count,
        time_taken.as_millis()
    );

    Ok(())
}

async fn add_tags_for_lens(db: &DatabaseTransaction, conf: &LensConfig) {
    // Tag domains
    for domain in &conf.domains {
        if let Err(err) = add_tags_for_url(db, &conf.name, domain).await {
            log::error!("Unable to add tags for {} - {}", domain, err);
        }
    }

    // Tag url prefixes
    for prefix in &conf.urls {
        let url = if prefix.ends_with('$') {
            prefix.trim_end_matches('$')
        } else {
            prefix.as_str()
        };

        if let Err(err) = add_tags_for_url(db, &conf.name, url).await {
            log::error!("Unable to add tags for {} - {}", url, err);
        }
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add crawl_tag table & idx
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                r#"CREATE TABLE IF NOT EXISTS "crawl_tag" (
                    "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                    "crawl_queue_id" integer NOT NULL,
                    "tag_id" integer NOT NULL,
                    "created_at" text NOT NULL,
                    "updated_at" text NOT NULL,
                    FOREIGN KEY(crawl_queue_id) REFERENCES crawl_queue(id),
                    FOREIGN KEY(tag_id)         REFERENCES tags(id)
                );"#
                .to_string(),
            ))
            .await?;

        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "CREATE UNIQUE INDEX IF NOT EXISTS `idx-crawl-tag-doc-id-tag-id` ON `crawl_tag` (`crawl_queue_id`, `tag_id`);"
                    .to_string(),
            ))
            .await?;

        let config = Config::new();
        let db = manager.get_connection();

        // Loop through lenses
        let lenses = lens::Entity::find()
            .filter(lens::Column::IsEnabled.eq(true))
            .filter(lens::Column::LensType.eq(LensType::Simple))
            .all(db)
            .await
            .unwrap_or_default();

        let lens_dir = config.lenses_dir();

        log::debug!("Loading lenses from {:?}", config.lenses_dir());
        for lens in lenses {
            let lens_path = lens_dir.join(format!("{}.ron", lens.name));
            if lens_path.exists() {
                match LensConfig::from_path(lens_path) {
                    Ok(lens_config) => {
                        let txn = db.begin().await?;
                        add_tags_for_lens(&txn, &lens_config).await;
                        txn.commit().await?;
                    }
                    Err(err) => log::error!("Unable to read lens: {}", err),
                }
            }
        }

        // Handle local files
        if let Err(err) = add_tags_for_url(db, "files", "file://").await {
            log::error!("Unable to add tags for file:// urls: {}", err);
        }

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
