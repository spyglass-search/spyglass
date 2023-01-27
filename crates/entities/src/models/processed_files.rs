use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use url::Url;

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

impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            created_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }

    // Triggered before insert / update
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        Ok(self)
    }
}

/// Helper method used to remove all documents that are not in the provided paths. This
/// is used to remove documents for folders that are no longer configured
pub async fn remove_unmatched_paths(
    db: &DatabaseConnection,
    paths: Vec<String>,
) -> anyhow::Result<Vec<Model>> {
    let mut find = Entity::find();
    if !paths.is_empty() {
        for path in paths {
            find = find.filter(Column::FilePath.not_like(format!("{}%", path).as_str()));
        }
    } else {
        log::debug!("No paths being watched removing all.");
    }

    match find.all(db).await {
        Ok(items) => {
            log::debug!("Removing {:?} unused files from the database.", items.len());
            let ids = items.iter().map(|model| model.id).collect::<Vec<i64>>();
            if let Err(error) = Entity::delete_many()
                .filter(Column::Id.is_in(ids))
                .exec(db)
                .await
            {
                log::error!("Error deleting unused paths {:?}", error);
                return Err(anyhow::Error::from(error));
            }
            Ok(items)
        }
        Err(error) => return Err(anyhow::Error::from(error)),
    }
}
