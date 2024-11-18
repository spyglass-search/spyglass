use sea_orm::{entity::prelude::*, InsertResult, Set};
use serde::Serialize;

use super::{indexed_document, vec_documents};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "vec_to_indexed")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub indexed_id: i64,
    /// When this was first added to the crawl queue.
    pub created_at: DateTimeUtc,
    /// When this task was last updated.
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    IndexedId,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::IndexedId => Entity::belongs_to(super::indexed_document::Entity)
                .from(Column::IndexedId)
                .to(super::indexed_document::Column::Id)
                .into(),
        }
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
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

pub async fn insert_embedding_mapping(
    db: &DatabaseConnection,
    indexed_id: i64,
) -> Result<InsertResult<ActiveModel>, DbErr> {
    let mut active_model = ActiveModel::new();
    active_model.indexed_id = Set(indexed_id);

    Entity::insert(active_model).exec(db).await
}

pub async fn delete_all_for_document(
    db: &DatabaseConnection,
    indexed_id: i64,
) -> Result<(), DbErr> {
    let documents = Entity::find()
        .filter(Column::IndexedId.eq(indexed_id))
        .all(db)
        .await?;

    if !documents.is_empty() {
        let ids = documents.iter().map(|val| val.id).collect::<Vec<i64>>();
        let _ = vec_documents::delete_embedding_by_ids(db, &ids).await?;

        let _ = Entity::delete_many()
            .filter(Column::Id.is_in(ids))
            .exec(db)
            .await;
        Ok(())
    } else {
        Ok(())
    }
}

pub async fn delete_all_by_urls(db: &DatabaseConnection, urls: &[String]) -> Result<(), DbErr> {
    let documents = indexed_document::Entity::find()
        .filter(indexed_document::Column::Url.is_in(urls))
        .all(db)
        .await?;

    for doc in documents {
        delete_all_for_document(db, doc.id).await?;
    }
    Ok(())
}
