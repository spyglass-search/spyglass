use crate::models::{document_tag, tag};
use sea_orm::entity::prelude::*;
use sea_orm::{FromQueryResult, InsertResult, QuerySelect, Set};

use super::tag::{get_or_create, TagPair};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "indexed_document")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Domain for this document, used to implement per domain crawl limits.
    pub domain: String,
    /// URL that was indexed.
    pub url: String,
    /// URL used to open in a file/browser window.
    pub open_url: Option<String>,
    /// Reference to the document in the index
    pub doc_id: String,
    /// When this was indexed
    pub created_at: DateTimeUtc,
    /// When this was last updated
    pub updated_at: DateTimeUtc,
}

impl Related<super::tag::Entity> for Entity {
    // The final relation is IndexedDocument -> DocumentTag -> Tag
    fn to() -> RelationDef {
        super::document_tag::Relation::Tag.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::document_tag::Relation::IndexedDocument.def().rev())
    }
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    Tag,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::Tag => Entity::has_many(tag::Entity).into(),
        }
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

impl ActiveModel {
    pub async fn insert_tags(
        &self,
        db: &DatabaseConnection,
        tags: &[TagPair],
    ) -> Result<InsertResult<document_tag::ActiveModel>, DbErr> {
        let mut tag_models: Vec<tag::Model> = Vec::new();
        for (label, value) in tags.iter() {
            if let Ok(tag) = get_or_create(db, label.to_owned(), value).await {
                tag_models.push(tag);
            }
        }

        // create connections for each tag
        let doc_tags = tag_models
            .iter()
            .map(|t| document_tag::ActiveModel {
                indexed_document_id: self.id.clone(),
                tag_id: Set(t.id),
                created_at: Set(chrono::Utc::now()),
                updated_at: Set(chrono::Utc::now()),
                ..Default::default()
            })
            .collect::<Vec<document_tag::ActiveModel>>();

        // Insert connections, ignoring duplicates
        document_tag::Entity::insert_many(doc_tags)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns(vec![
                    document_tag::Column::IndexedDocumentId,
                    document_tag::Column::TagId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(db)
            .await
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
    use crate::models::{document_tag, tag};
    use crate::test::setup_test_db;
    use sea_orm::{ActiveModelTrait, DbErr, EntityTrait, ModelTrait, Set};

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

    #[tokio::test]
    async fn test_document_tag_support() -> Result<(), DbErr> {
        let db = setup_test_db().await;

        let doc = super::ActiveModel {
            domain: Set("en.wikipedia.com".into()),
            url: Set("https://en.wikipedia.org/wiki/Rust_(programming_language)".into()),
            doc_id: Set("1".into()),
            ..Default::default()
        };
        let doc = doc.save(&db).await.unwrap();

        // Insert related tags
        let tags = vec![
            (tag::TagType::Source, "web".to_owned()),
            // Should only add one of these.
            (tag::TagType::MimeType, "text/html".to_owned()),
            (tag::TagType::MimeType, "text/html".to_owned()),
        ];

        if let Err(res) = doc.insert_tags(&db, &tags).await {
            dbg!(res);
        }

        let res = document_tag::Entity::find().all(&db).await?;
        assert_eq!(res.len(), 2);

        let doc_res = super::Entity::find_by_id(doc.id.clone().unwrap())
            .one(&db)
            .await?
            .unwrap();

        let doc_tags = doc_res.find_related(tag::Entity).all(&db).await?;
        assert_eq!(doc_res.id, doc.id.unwrap());
        assert_eq!(doc_tags.len(), 2);
        Ok(())
    }
}
