use std::collections::HashSet;
use std::ops::Sub;

use crate::models::{document_tag, tag};
use crate::BATCH_SIZE;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ConnectionTrait, FromQueryResult, InsertResult, QuerySelect, QueryTrait, Set,
    Statement,
};
use serde::Serialize;

use super::tag::{get_or_create, TagPair};

#[derive(Clone, Debug, Serialize, PartialEq, DeriveEntityModel, Eq)]
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

impl Model {
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
                indexed_document_id: Set(self.id),
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

pub async fn insert_many(db: &impl ConnectionTrait, docs: &[ActiveModel]) -> Result<(), DbErr> {
    for insert_chunk in docs.chunks(BATCH_SIZE) {
        Entity::insert_many(insert_chunk.to_vec())
            .on_conflict(
                OnConflict::columns(vec![Column::Url])
                    .update_column(Column::UpdatedAt)
                    .to_owned(),
            )
            .exec(db)
            .await?;
    }

    Ok(())
}

pub async fn insert_tags_for_docs<C: ConnectionTrait>(
    db: &C,
    docs: &[Model],
    tags: &[i64],
) -> Result<(), DbErr> {
    // Nothing to do if we have no docs or tags
    if docs.is_empty() || tags.is_empty() {
        return Ok(());
    }

    let doc_ids: Vec<i64> = docs.iter().map(|m| m.id).collect();

    insert_tags_for_docs_by_id(db, &doc_ids, tags, true).await
}

/// Creates connections between a set of tags and a set of documents
/// The document its provided are the document db id field and the
/// tag ids are the tags database id field.
///
/// The remove unused option is utilized to remove any links to tags
/// that are not in the list. This can be used when replacing the
/// current set of tags with the passed in set
pub async fn insert_tags_for_docs_by_id<C: ConnectionTrait>(
    db: &C,
    doc_ids: &[i64],
    tags: &[i64],
    remove_unused: bool,
) -> Result<(), DbErr> {
    // Nothing to do if we have no docs or tags
    if doc_ids.is_empty() || tags.is_empty() {
        return Ok(());
    }

    // Remove dupes before adding
    let tags: HashSet<i64> = HashSet::from_iter(tags.iter().cloned());
    let doc_ids: Vec<i64> = doc_ids.to_vec();

    if remove_unused {
        // Remove tags that are not in the tag set
        let _ = document_tag::Entity::delete_many()
            .filter(document_tag::Column::IndexedDocumentId.is_in(doc_ids.clone()))
            .filter(document_tag::Column::TagId.is_not_in(tags.clone()))
            .exec(db)
            .await;
    }

    // Grab existing tags
    let existing_tags = document_tag::Entity::find()
        .filter(document_tag::Column::IndexedDocumentId.is_in(doc_ids.clone()))
        .all(db)
        .await
        .unwrap_or_default()
        .iter()
        .map(|model| model.tag_id)
        .collect::<HashSet<_>>();

    // Only add tags that have not been added before.
    let tags = tags.sub(&existing_tags);
    // create connections for each tag
    let doc_tags = doc_ids
        .iter()
        .flat_map(|id| {
            tags.iter().map(|t| document_tag::ActiveModel {
                indexed_document_id: Set(*id),
                tag_id: Set(*t),
                created_at: Set(chrono::Utc::now()),
                updated_at: Set(chrono::Utc::now()),
                ..Default::default()
            })
        })
        .collect::<Vec<document_tag::ActiveModel>>();

    // Nothing to add, great!
    if doc_tags.is_empty() {
        return Ok(());
    }

    // Insert connections, ignoring duplicates
    for chunk in doc_tags.chunks(BATCH_SIZE) {
        let query = document_tag::Entity::insert_many(chunk.to_owned())
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns(vec![
                    document_tag::Column::IndexedDocumentId,
                    document_tag::Column::TagId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .build(db.get_database_backend());

        if let Err(err) = db.execute(query.clone()).await {
            log::error!("Unable to execute: {} due to {}", query.to_string(), err);
            return Err(err);
        }
    }

    Ok(())
}

/// Removes the specified tags from the specified documents. The ids for the
/// tags and documents are the database id fields
pub async fn remove_tags_for_docs_by_id<C: ConnectionTrait>(
    db: &C,
    doc_ids: &[i64],
    tags: &[i64],
) -> Result<(), DbErr> {
    // Remove specified tags
    let _ = document_tag::Entity::delete_many()
        .filter(document_tag::Column::IndexedDocumentId.is_in(doc_ids.to_vec()))
        .filter(document_tag::Column::TagId.is_in(tags.to_vec()))
        .exec(db)
        .await;

    Ok(())
}

/// Inserts an entry into the tag table for each document and
/// tag pair provided
pub async fn insert_tags_many<C: ConnectionTrait>(
    db: &C,
    docs: &[Model],
    tags: &[TagPair],
) -> Result<(), DbErr> {
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
    let mut num_deleted = 0;
    for chunk in dbids.chunks(BATCH_SIZE) {
        let res = Entity::delete_many()
            .filter(Column::Id.is_in(chunk.to_owned()))
            .exec(db)
            .await?;
        num_deleted += res.rows_affected;
    }

    Ok(num_deleted)
}

/// Helper method used to delete multiple documents by url. This method will first
/// delete all related tag references before deleting the documents
pub async fn delete_many_by_url(
    db: &DatabaseConnection,
    urls: &[String],
) -> Result<u64, sea_orm::DbErr> {
    let mut num_deleted = 0;
    for chunk in urls.chunks(BATCH_SIZE) {
        let entries = Entity::find()
            .filter(Column::Url.is_in(chunk))
            .all(db)
            .await?;

        let id_list = entries.iter().map(|entry| entry.id).collect::<Vec<i64>>();

        num_deleted += delete_many_by_id(db, &id_list).await?;
    }

    Ok(num_deleted)
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
        db.get_database_backend(),
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

/// Helper method used to access the documents database id field from the
/// string document id
pub async fn find_by_doc_ids(
    db: &DatabaseConnection,
    ids: &[String],
) -> Result<Vec<IndexedDocumentId>, sea_orm::DbErr> {
    let doc_ids = ids
        .iter()
        .map(|str| format!("\"{str}\""))
        .collect::<Vec<String>>()
        .join(",");

    IndexedDocumentId::find_by_statement(Statement::from_string(
        db.get_database_backend(),
        format!(
            r#"
        SELECT
            id,
            doc_id
        FROM indexed_document
        WHERE doc_id in ({})"#,
            doc_ids
        ),
    ))
    .all(db)
    .await
}

/// Represents the tag id that is associated with a document
#[derive(Debug, FromQueryResult)]
pub struct IndexedDocumentTagId {
    pub id: i64,
}

/// Helper method used to access the database ids of the tags associated with the
/// specified document. The passed in document id is the string document id field.
pub async fn get_tag_ids_by_doc_id(
    db: &DatabaseConnection,
    id: &str,
) -> Result<Vec<IndexedDocumentTagId>, sea_orm::DbErr> {
    IndexedDocumentTagId::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT
            document_tag.tag_id as id
        FROM document_tag as document_tag
        LEFT JOIN indexed_document as indexed_doc on document_tag.indexed_document_id = indexed_doc.id
        WHERE indexed_doc.doc_id = $1"#,
        vec![id.into()],
    ))
    .all(db)
    .await
}

pub enum DocumentIdentifier<'a> {
    DocId(&'a str),
    Url(&'a str),
}

pub async fn get_document_details(
    db: &DatabaseConnection,
    identifier: DocumentIdentifier<'_>,
) -> Result<Option<(Model, Vec<tag::Model>)>, DbErr> {
    let query = Entity::find();
    let query = match identifier {
        DocumentIdentifier::DocId(doc_id) => query.filter(Column::DocId.eq(doc_id)),
        DocumentIdentifier::Url(url) => query.filter(Column::Url.eq(url)),
    };

    if let Some(doc) = query.one(db).await? {
        let tags = doc
            .find_related(tag::Entity)
            .all(db)
            .await
            .unwrap_or_default();
        return Ok(Some((doc, tags)));
    }
    Ok(None)
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
    use std::collections::HashMap;

    use crate::models::document_tag;
    use crate::models::indexed_document::insert_tags_for_docs;
    use crate::models::tag::{self, TagType};
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
        let doc = doc.insert(&db).await.unwrap();

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

        let doc_res = super::Entity::find_by_id(doc.id.clone())
            .one(&db)
            .await?
            .unwrap();

        let doc_tags = doc_res.find_related(tag::Entity).all(&db).await?;
        assert_eq!(doc_res.id, doc.id);
        assert_eq!(doc_tags.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn test_insert_tags_for_dcs() {
        let db = setup_test_db().await;
        let doc = super::ActiveModel {
            domain: Set("en.wikipedia.com".into()),
            url: Set("https://en.wikipedia.org/wiki/Rust_(programming_language)".into()),
            doc_id: Set("1".into()),
            ..Default::default()
        };

        let doc = doc.insert(&db).await.expect("Unable to add doc");
        let tags = vec![
            (TagType::Lens, "lens".to_owned()),
            (TagType::Source, "original".to_owned()),
            (TagType::Type, "remove".to_string()),
        ];
        let _ = doc.insert_tags(&db, &tags).await;

        // Grab the original tags to compare against the update dones
        let tags_before = document_tag::Entity::find()
            .all(&db)
            .await
            .expect("Unable to grab tags")
            .iter()
            .map(|x| (x.tag_id, x.to_owned()))
            .collect::<HashMap<_, _>>();
        assert_eq!(tags_before.len(), 3);

        let tags = vec![
            // kept the same.
            (TagType::Lens, "lens".to_owned()),
            // updated from original
            (TagType::Source, "new_source".to_owned()),
            // removed type tag.
        ];
        let tags = tag::get_or_create_many(&db, &tags)
            .await
            .expect("Unable to get/create tags")
            .iter()
            .map(|m| m.id)
            .collect::<Vec<_>>();

        assert!(insert_tags_for_docs(&db, &[doc], &tags).await.is_ok());
        let tags_after = document_tag::Entity::find()
            .all(&db)
            .await
            .expect("Unable to grab tags")
            .iter()
            .map(|x| (x.tag_id, x.to_owned()))
            .collect::<HashMap<_, _>>();
        assert_eq!(tags_after.len(), 2);

        for (id, model) in tags_before.iter() {
            // The same tag should not have been changed in anyway
            if let Some(model_after) = tags_after.get(&id) {
                assert_eq!(model.id, model_after.id);
            }
        }
    }
}
