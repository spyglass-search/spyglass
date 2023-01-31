use entities::{
    BATCH_SIZE,
    models::{
        document_tag, indexed_document,
        lens::{self, LensType},
        tag::{get_or_create, TagType},
    },
    sea_orm::{
        ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set,
        TransactionTrait,
    },
};
use sea_orm_migration::prelude::*;
use shared::config::{Config, LensConfig};
use std::time::Instant;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20221124_000001_add_tags_for_existing_lenses"
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

    // Update existing documents
    let start_time = Instant::now();
    let existing_docs = indexed_document::Entity::find()
        .filter(indexed_document::Column::Url.contains(url))
        .all(tx)
        .await?;

    let tag = get_or_create(tx, TagType::Lens, name).await?;
    // create connections for each tag
    let doc_tags = existing_docs
        .iter()
        .map(|doc| document_tag::ActiveModel {
            indexed_document_id: Set(doc.id),
            tag_id: Set(tag.id),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..Default::default()
        })
        .collect::<Vec<document_tag::ActiveModel>>();

    // Insert connections, ignoring duplicates
    for chunk in doc_tags.chunks(BATCH_SIZE) {
        document_tag::Entity::insert_many(chunk.to_vec())
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns(vec![
                    document_tag::Column::IndexedDocumentId,
                    document_tag::Column::TagId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(tx)
            .await?;
    }

    let count = existing_docs.len();
    let time_taken = Instant::now() - start_time;
    log::info!(
        "{}: tagged {} docs in {}ms",
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
