use std::collections::HashSet;

use crate::models::{document_tag, tag};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ConnectionTrait, DatabaseBackend, FromQueryResult, InsertResult, QuerySelect, Set, Statement,
};

use super::tag::{get_or_create, TagPair};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "indexed_document")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Domain for this document, used to implement per domain crawl limits.
    pub domain: String,
    /// URL that was indexed.
    #[sea_orm(unique)]
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
    pub async fn insert_tags<C: ConnectionTrait>(
        &self,
        db: &C,
        tags: &[TagPair],
    ) -> Result<InsertResult<document_tag::ActiveModel>, DbErr> {
        let mut tag_models: Vec<tag::Model> = Vec::new();
        for (label, value) in tags.iter() {
            match get_or_create(db, label.to_owned(), value).await {
                Ok(tag) => tag_models.push(tag),
                Err(err) => log::error!("{}", err),
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

pub async fn insert_many(
    db: &impl ConnectionTrait,
    docs: Vec<ActiveModel>,
) -> Result<InsertResult<ActiveModel>, DbErr> {
    Entity::insert_many(docs)
        .on_conflict(
            OnConflict::columns(vec![Column::Url])
                .update_column(Column::UpdatedAt)
                .to_owned(),
        )
        .exec(db)
        .await
}

pub async fn insert_tags_for_docs<C: ConnectionTrait>(
    db: &C,
    docs: &[Model],
    tags: &[i64],
) -> Result<InsertResult<document_tag::ActiveModel>, DbErr> {
    // Remove dupes before adding
    let tags: HashSet<i64> = HashSet::from_iter(tags.iter().cloned());
    let doc_ids: Vec<i64> = docs.iter().map(|m| m.id).collect();

    // Remove existing tags for doc
    let _ = document_tag::Entity::delete_many()
        .filter(document_tag::Column::IndexedDocumentId.is_in(doc_ids))
        .exec(db)
        .await;

    // create connections for each tag
    let doc_tags = docs
        .iter()
        .flat_map(|model| {
            tags.iter().map(|t| document_tag::ActiveModel {
                indexed_document_id: Set(model.id),
                tag_id: Set(*t),
                created_at: Set(chrono::Utc::now()),
                updated_at: Set(chrono::Utc::now()),
                ..Default::default()
            })
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

/// Inserts an entry into the tag table for each document and
/// tag pair provided
pub async fn insert_tags_many<C: ConnectionTrait>(
    db: &C,
    docs: &[Model],
    tags: &[TagPair],
) -> Result<InsertResult<document_tag::ActiveModel>, DbErr> {
    let mut tag_ids: Vec<i64> = Vec::new();
    for (label, value) in tags.iter() {
        match get_or_create(db, label.to_owned(), value).await {
            Ok(tag) => tag_ids.push(tag.id),
            Err(err) => log::error!("{}", err),
        }
    }

    insert_tags_for_docs(db, docs, &tag_ids).await
}

/// Remove documents from the indexed_document table that match `rule`. Rule is expected
/// to be a SQL like statement.
pub async fn delete_by_rule(db: &DatabaseConnection, rule: &str) -> anyhow::Result<Vec<String>> {
    let matching = Entity::find()
        .filter(Column::Url.like(rule))
        .all(db)
        .await?;

    let removed = matching
        .iter()
        .map(|x| (x.id, x.doc_id.to_string()))
        .collect::<Vec<(i64, String)>>();

    if !removed.is_empty() {
        let ids = removed.iter().map(|(id, _)| *id).collect::<Vec<i64>>();
        delete_many_by_id(db, &ids).await?;
        log::info!("removed {} docs due to '{}'", removed.len(), rule);
    }

    Ok(removed
        .into_iter()
        .map(|(_id, doc_id)| doc_id)
        .collect::<Vec<String>>())
}

/// Helper method used to delete multiple documents by id. This method will first
/// delete all related tag references before deleting the documents
pub async fn delete_many_by_id(
    db: &DatabaseConnection,
    dbids: &[i64],
) -> Result<u64, sea_orm::DbErr> {
    // Delete all associated tags
    document_tag::Entity::delete_many()
        .filter(document_tag::Column::IndexedDocumentId.is_in(dbids.to_owned()))
        .exec(db)
        .await?;

    // Delete item
    let res = Entity::delete_many()
        .filter(Column::Id.is_in(dbids.to_owned()))
        .exec(db)
        .await?;

    Ok(res.rows_affected)
}

/// Helper method used to delete multiple documents by url. This method will first
/// delete all related tag references before deleting the documents
pub async fn delete_many_by_url(
    db: &DatabaseConnection,
    urls: Vec<String>,
) -> Result<u64, sea_orm::DbErr> {
    let entries = Entity::find()
        .filter(Column::Url.is_in(urls))
        .all(db)
        .await?;

    let id_list = entries.iter().map(|entry| entry.id).collect::<Vec<i64>>();

    delete_many_by_id(db, &id_list).await
}

#[derive(Debug, FromQueryResult)]
pub struct IndexedDocumentId {
    pub id: i64,
    pub doc_id: String,
}

pub async fn find_by_lens(
    db: DatabaseConnection,
    name: &str,
) -> Result<Vec<IndexedDocumentId>, sea_orm::DbErr> {
    IndexedDocumentId::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        r#"
        SELECT
            indexed_document.id,
            indexed_document.doc_id
        FROM indexed_document
        LEFT JOIN document_tag on indexed_document.id = document_tag.indexed_document_id
        LEFT JOIN tags on tags.id = document_tag.tag_id
        WHERE tags.label = "lens" AND tags.value = $1"#,
        vec![name.into()],
    ))
    .all(&db)
    .await
}

#[cfg(test)]
mod test {
    use crate::models::{document_tag, tag};
    use crate::test::setup_test_db;
    use sea_orm::{ActiveModelTrait, DbErr, EntityTrait, ModelTrait, Set};

    #[tokio::test]
    async fn test_delete_by_rule() {
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

        let removed = super::delete_by_rule(&db, "https://en.wikipedia.com/%action=%")
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
