use sea_orm::{entity::prelude::*, Set};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "bootstrap_queue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Name of lens that was bootstrapped.
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
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert {
            self.updated_at = Set(chrono::Utc::now());
        }

        Ok(self)
    }
}

pub async fn is_bootstrapped(
    db: &DatabaseConnection,
    lens_name: &str,
) -> anyhow::Result<bool, sea_orm::DbErr> {
    let res = Entity::find()
        .filter(Column::SeedUrl.eq(lens_name))
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

// Helper method to copy the table from one database to another
pub async fn copy_table(
    from: &DatabaseConnection,
    to: &DatabaseConnection,
) -> anyhow::Result<(), sea_orm::DbErr> {
    let mut pages = Entity::find().paginate(from, 1000);
    Entity::delete_many().exec(to).await?;
    while let Ok(Some(pages)) = pages.fetch_and_next().await {
        let active_model = pages
            .into_iter()
            .map(|model| model.into())
            .collect::<Vec<ActiveModel>>();
        Entity::insert_many(active_model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns(vec![Column::Id])
                    .do_nothing()
                    .to_owned(),
            )
            .exec(to)
            .await?;
    }
    Ok(())
}
