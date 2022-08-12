use sea_orm::{entity::prelude::*, Set};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "bootstrap_queue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Domain/URL prefixed that was used to bootstrap
    #[sea_orm(unique)]
    pub seed_url: String,
    /// Number of URLs added to the crawl queue
    pub count: i64,
    /// When this was first added
    pub created_at: DateTimeUtc,
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

pub async fn has_seed_url(
    db: &DatabaseConnection,
    seed_url: &str,
) -> anyhow::Result<bool, sea_orm::DbErr> {
    let res = Entity::find()
        .filter(Column::SeedUrl.eq(seed_url))
        .one(db)
        .await?;

    Ok(res.is_some())
}

/// Keep track of the seed_url used
pub async fn enqueue(
    db: &DatabaseConnection,
    seed_url: &str,
    count: i64,
) -> anyhow::Result<(), sea_orm::DbErr> {
    let new_row = ActiveModel {
        seed_url: Set(seed_url.to_string()),
        count: Set(count),
        ..Default::default()
    };

    new_row.insert(db).await?;
    Ok(())
}

pub async fn dequeue(
    db: &DatabaseConnection,
    seed_url: &str,
) -> anyhow::Result<(), sea_orm::DbErr> {
    let res = Entity::find()
        .filter(Column::SeedUrl.eq(seed_url))
        .one(db)
        .await?;

    if let Some(res) = res {
        res.delete(db).await?;
    }

    Ok(())
}
