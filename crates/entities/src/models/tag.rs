use sea_orm::Set;
use sea_orm::{entity::prelude::*, ConnectionTrait};
use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, EnumString};

use super::indexed_document;

pub type TagPair = (TagType, String);

#[derive(
    AsRefStr,
    Clone,
    Debug,
    DeriveActiveEnum,
    Deserialize,
    EnumIter,
    EnumString,
    Eq,
    Hash,
    PartialEq,
    Serialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(None)")]
pub enum TagType {
    // Marked as liked/starred/hearted/etc.
    #[sea_orm(string_value = "favorited")]
    Favorited,
    // Mimetype of the document. TODO: Need to keep a mapping between file extension and
    // mimetypes somewhere
    #[sea_orm(string_value = "mimetype")]
    MimeType,
    // where this document came from,
    #[sea_orm(string_value = "source")]
    Source,
    // Owner of a doc/item, if relevant.
    #[sea_orm(string_value = "owner")]
    Owner,
    // Shared/invited to a doc/event/etc.
    #[sea_orm(string_value = "shared")]
    SharedWith,
    // Part of this/these lens(es)
    #[sea_orm(string_value = "lens")]
    Lens,
}

#[derive(AsRefStr)]
pub enum TagValue {
    #[strum(serialize = "favorited")]
    Favorited,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "tags")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub label: TagType,
    pub value: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    IndexedDocument,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::IndexedDocument => Entity::has_many(indexed_document::Entity).into(),
        }
    }
}

impl ActiveModelBehavior for ActiveModel {
    // Triggered before insert / update
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        if insert {
            self.created_at = Set(chrono::Utc::now());
            self.updated_at = Set(chrono::Utc::now());
        } else {
            self.updated_at = Set(chrono::Utc::now());
        }

        Ok(self)
    }
}

impl Related<super::indexed_document::Entity> for Entity {
    // The final relation is IndexedDocument -> DocumentTag -> Tag
    fn to() -> RelationDef {
        super::document_tag::Relation::Tag.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::document_tag::Relation::Tag.def().rev())
    }
}

pub async fn get_or_create<C>(db: &C, label: TagType, value: &str) -> Result<Model, DbErr>
where
    C: ConnectionTrait,
{
    let tag = ActiveModel {
        label: Set(label.clone()),
        value: Set(value.to_string()),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
        ..Default::default()
    };

    let _ = Entity::insert(tag)
        .on_conflict(
            sea_orm::sea_query::OnConflict::columns(vec![Column::Label, Column::Value])
                .do_nothing()
                .to_owned(),
        )
        .exec_with_returning(db)
        .await;

    let tag = Entity::find()
        .filter(Column::Label.eq(label.clone()))
        .filter(Column::Value.eq(value))
        .one(db)
        .await;

    match tag {
        Ok(Some(model)) => Ok(model),
        Err(err) => Err(err),
        _ => Err(DbErr::RecordNotFound(format!(
            "label: {}, value: {}",
            label, value
        ))),
    }
}

#[cfg(test)]
mod test {
    use crate::models::tag;
    use crate::test::setup_test_db;
    use sea_orm::{DbErr, EntityTrait, Set};

    #[tokio::test]
    async fn test_add_or_create() -> Result<(), DbErr> {
        let db = setup_test_db().await;
        let new_tag = super::get_or_create(&db, tag::TagType::Source, "web").await?;
        let expected_id = new_tag.id;

        let new_tag = super::get_or_create(&db, tag::TagType::Source, "web").await?;
        assert_eq!(expected_id, new_tag.id);
        Ok(())
    }

    #[tokio::test]
    async fn test_conflict() -> Result<(), DbErr> {
        let db = setup_test_db().await;
        let source_tag = tag::ActiveModel {
            label: Set(tag::TagType::Source),
            value: Set("web".to_string()),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..Default::default()
        };

        let mime_tag = tag::ActiveModel {
            label: Set(tag::TagType::MimeType),
            value: Set("text/html".to_string()),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..Default::default()
        };

        let conflict = tag::ActiveModel {
            label: Set(tag::TagType::MimeType),
            value: Set("text/html".to_string()),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..Default::default()
        };

        let tags = vec![source_tag, mime_tag, conflict];
        let _ = tag::Entity::insert_many(tags.clone())
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns(vec![
                    tag::Column::Label,
                    tag::Column::Value,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(&db)
            .await?;

        let tags = tag::Entity::find().all(&db).await?;
        assert_eq!(tags.len(), 2);

        Ok(())
    }
}
