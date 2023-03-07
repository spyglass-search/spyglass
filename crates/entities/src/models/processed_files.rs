use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseBackend, FromQueryResult, Set, Statement};
use serde::Serialize;

use crate::BATCH_SIZE;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "processed_files")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// URL to crawl
    #[sea_orm(unique)]
    pub file_path: String,

    /// When this was first added to the crawl queue.
    pub created_at: DateTimeUtc,
    /// When this task was last updated.
    pub last_modified: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No RelationDef")
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            created_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }

    // Triggered before insert / update
    async fn before_save<C>(mut self, _db: &C, _insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        Ok(self)
    }
}

/// Helper method used to remove all documents that are not in the provided paths. This
/// is used to remove documents for folders that are no longer configured
pub async fn remove_unmatched_paths(
    db: &DatabaseConnection,
    paths: Vec<String>,
) -> anyhow::Result<Vec<Model>> {
    if !paths.is_empty() {
        log::debug!("removing files not matching {:?}", paths);
        let mut find = Entity::find();
        for path in paths {
            find = find.filter(Column::FilePath.not_like(format!("{path}%").as_str()));
        }

        let items = find.all(db).await?;
        log::debug!("Removing {:?} unused files from the database.", items.len());
        let ids = items.iter().map(|model| model.id).collect::<Vec<i64>>();
        for chunk in ids.chunks(BATCH_SIZE) {
            if let Err(error) = Entity::delete_many()
                .filter(Column::Id.is_in(chunk.to_vec()))
                .exec(db)
                .await
            {
                log::warn!("Error deleting unused paths {:?}", error);
            }
        }

        Ok(items)
    } else {
        log::debug!("No paths being watched removing all.");
        Ok(Vec::new())
    }
}

#[derive(Debug, FromQueryResult)]
struct FileUrls {
    pub url: String,
}

pub async fn get_files_to_recrawl(
    ext: &str,
    db: &DatabaseConnection,
) -> Result<Vec<String>, DbErr> {
    let ext_filter = format!("%.{ext}");
    let urls = FileUrls::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        r#"
        with possible as (
            select url
            from crawl_queue
             where url like $1
        )
        select file_path as url
        from processed_files
            where file_path like $1 and file_path not in possible;"#,
        vec![ext_filter.into()],
    ))
    .all(db)
    .await;

    match urls {
        Ok(urls) => Ok(urls.iter().map(|file| file.url.clone()).collect()),
        Err(err) => Err(err),
    }
}
