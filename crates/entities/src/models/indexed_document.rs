use sea_orm::entity::prelude::*;
use sea_orm::{FromQueryResult, QuerySelect, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "indexed_document")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Domain for this document, used to implement per domain crawl limits.
    pub domain: String,
    /// URL that was indexed
    pub url: String,
    /// Reference to the document in the index
    pub doc_id: String,
    /// When this was indexed
    pub created_at: DateTimeUtc,
    /// When this was last updated
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

#[derive(Debug, FromQueryResult)]
pub struct CountByDomain {
    pub count: i64,
    pub domain: String,
}

pub async fn indexed_stats(
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<CountByDomain>, sea_orm::DbErr> {
    let res = Entity::find()
        .column_as(Column::Id.count(), "count")
        .column(Column::Domain)
        .group_by(Column::Domain)
        .into_model::<CountByDomain>()
        .all(db)
        .await?;

    Ok(res)
}

/// Remove documents from the indexed_document table that match `rule`. Rule is expected
/// to be a SQL like statement.
pub async fn remove_by_rule(db: &DatabaseConnection, rule: &str) -> anyhow::Result<Vec<String>> {
    let matching = Entity::find()
        .filter(Column::Url.like(rule))
        .all(db)
        .await?;

    let removed = matching
        .iter()
        .map(|x| x.doc_id.to_string())
        .collect::<Vec<String>>();

    let _ = Entity::delete_many()
        .filter(Column::Url.like(rule))
        .exec(db)
        .await?;

    if !removed.is_empty() {
        log::info!("removed {} docs due to '{}'", removed.len(), rule);
    }
    Ok(removed)
}

#[cfg(test)]
mod test {
    use crate::test::setup_test_db;
    use sea_orm::{ActiveModelTrait, Set};

    #[tokio::test]
    async fn test_remove_by_rule() {
        let db = setup_test_db().await;

        let doc = super::ActiveModel {
            domain: Set("en.wikipedia.com".into()),
            url: Set("https://en.wikipedia.org/wiki/Rust_(programming_language)".into()),
            doc_id: Set("1".into()),
            ..Default::default()
        };
        doc.save(&db).await.unwrap();
        let doc = super::ActiveModel {
            domain: Set("en.wikipedia.com".into()),
            url: Set("https://en.wikipedia.com/wiki/Cheese?id=13314&action=edit".into()),
            doc_id: Set("1".into()),
            ..Default::default()
        };
        doc.save(&db).await.unwrap();

        let removed = super::remove_by_rule(&db, "https://en.wikipedia.com/%action=%")
            .await
            .unwrap();
        assert_eq!(removed.len(), 1);
    }
}
