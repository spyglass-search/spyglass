use sea_orm::entity::prelude::*;
use sea_orm::Set;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
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

impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }
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
