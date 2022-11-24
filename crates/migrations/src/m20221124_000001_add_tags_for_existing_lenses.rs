use entities::{
    models::{
        crawl_queue::{self, TaskData},
        indexed_document,
        lens::{self, LensType},
        tag::TagType,
    },
    sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set},
};
use sea_orm_migration::prelude::*;
use shared::config::{Config, LensConfig};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20221124_000001_add_tags_for_existing_lenses"
    }
}

async fn add_tags_for_url(db: &DatabaseConnection, name: &str, url: &str) -> Result<(), DbErr> {
    // apply lens tag to existing & new urls
    let tag = vec![(TagType::Lens, name.to_owned())];
    let existing_tasks = crawl_queue::Entity::find()
        .filter(crawl_queue::Column::Url.starts_with(url))
        .all(db)
        .await?;

    // Update existing tasks
    let data = TaskData::new(&tag);
    let count = existing_tasks.len();
    for task in existing_tasks {
        let mut active: crawl_queue::ActiveModel = task.clone().into();
        if let Some(old) = task.data {
            active.data = Set(Some(data.merge(&old)));
        } else {
            active.data = Set(Some(data.clone()));
        }
        active.save(db).await?;
    }
    log::info!("{}: tagged {} tasks", name, count);

    // Update existing documents
    let existing_docs = indexed_document::Entity::find()
        .filter(indexed_document::Column::Url.starts_with(url))
        .all(db)
        .await?;
    let count = existing_docs.len();
    for doc in existing_docs {
        let model: indexed_document::ActiveModel = doc.into();
        model.insert_tags(db, &tag).await?;
    }
    log::info!("{}: tagged {} docs", name, count);

    Ok(())
}

async fn add_tags_for_lens(db: &DatabaseConnection, conf: &LensConfig) {
    // Tag domains
    for domain in &conf.domains {
        let seed_url = format!("https://{}", domain);
        if let Err(err) = add_tags_for_url(db, &conf.name, &seed_url).await {
            log::error!("Unable to add task tags for {} - {}", seed_url, err);
        }
    }

    // Tag url prefixes
    for prefix in &conf.urls {
        let url = if prefix.ends_with('$') {
            prefix.strip_suffix('$').expect("No $ at end of prefix")
        } else {
            prefix.as_str()
        };

        if let Err(err) = add_tags_for_url(db, &conf.name, url).await {
            log::error!("Unable to add doc tags for {} - {}", url, err);
        }
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                    Ok(lens_config) => add_tags_for_lens(db, &lens_config).await,
                    Err(err) => log::error!("Unable to read lens: {}", err),
                }
            }
        }

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
