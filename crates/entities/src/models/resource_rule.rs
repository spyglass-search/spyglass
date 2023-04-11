use sea_orm::entity::prelude::*;
use sea_orm::Set;

#[derive(Clone, Debug, Eq, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "resource_rules")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub domain: String,
    pub rule: String,
    pub no_index: bool,
    pub allow_crawl: bool,
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

#[cfg(test)]
mod test {
    use sea_orm::prelude::*;
    use sea_orm::{ActiveModelTrait, ColumnTrait, Set};

    use crate::models::resource_rule;
    use crate::test::setup_test_db;

    #[tokio::test]
    async fn test_insert() -> anyhow::Result<(), sea_orm::DbErr> {
        let db = setup_test_db().await;

        let domain = "oldschool.runescape.wiki";
        let rule = "/";

        let new_rule = resource_rule::ActiveModel {
            domain: Set(domain.to_owned()),
            rule: Set(rule.to_owned()),
            no_index: Set(false),
            allow_crawl: Set(true),
            ..Default::default()
        };
        new_rule.insert(&db).await.expect("Unable to insert");

        let query = resource_rule::Entity::find()
            .filter(resource_rule::Column::Domain.eq(domain))
            .all(&db)
            .await
            .expect("Unable to run query");

        assert_eq!(query.len(), 1);

        Ok(())
    }
}
