use sea_orm::{entity::prelude::*, Condition, ConnectionTrait, Set};
use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, Display, EnumString};

use super::{crawl_queue, indexed_document};

pub type TagPair = (TagType, String);

#[derive(
    AsRefStr, Clone, Debug, Deserialize, EnumIter, EnumString, Eq, Hash, PartialEq, Serialize,
)]
pub enum TagType {
    /// Marked as liked/starred/hearted/etc.
    #[strum(serialize = "favorited")]
    Favorited,
    /// Mimetype of the document. TODO: Need to keep a mapping between file extension and
    /// mimetypes somewhere
    #[strum(serialize = "mimetype")]
    MimeType,
    /// General type tag, Used for high level types ex: File, directory. The MimeType
    /// would be used as a more specific type.
    /// For non-file docs, can be used to differentiate from others in this category.
    /// e.g. for a GitHub connection we can have an "Issue" or "Repo".
    ///     for a D&D lens we have equipment, magic items, skills, etc.
    #[strum(serialize = "type")]
    Type,
    /// where this document came from,
    #[strum(serialize = "source")]
    Source,
    /// Owner of a doc/item, if relevant.
    #[strum(serialize = "owner")]
    Owner,
    /// Shared/invited to a doc/event/etc.
    #[strum(serialize = "shared")]
    SharedWith,
    /// Part of this/these lens(es)
    #[strum(serialize = "lens")]
    Lens,
    /// Part of a specific repo
    #[strum(serialize = "repository")]
    Repository,
    /// For file based content this tag
    #[strum(serialize = "fileext")]
    FileExt,
    /// Pull from the lens categorization
    #[strum(serialize = "category")]
    Category,
    /// Other custom generated TagTypes.
    #[strum(serialize = "Other(String)")]
    Other(String),
}

impl TagType {
    pub fn string_to_tag_type(v: &str) -> Self {
        string_to_tag_type(v)
    }
}

// Helper method used to convert a string into the
// associate TagType
fn string_to_tag_type(v: &str) -> TagType {
    match v {
        "favorited" => TagType::Favorited,
        "mimetype" => TagType::MimeType,
        "type" => TagType::Type,
        "source" => TagType::Source,
        "owner" => TagType::Owner,
        "shared" => TagType::SharedWith,
        "lens" => TagType::Lens,
        "repository" => TagType::Repository,
        "fileext" => TagType::FileExt,
        "category" => TagType::Category,
        other => TagType::Other(String::from(other)),
    }
}

// Allows the TagType to be converted into a string
impl ToString for TagType {
    fn to_string(&self) -> String {
        match self {
            Self::Favorited => "favorited",
            Self::MimeType => "mimetype",
            Self::Type => "type",
            Self::Source => "source",
            Self::Owner => "owner",
            Self::SharedWith => "shared",
            Self::Lens => "lens",
            Self::Repository => "repository",
            Self::FileExt => "fileext",
            Self::Category => "category",
            Self::Other(label) => label.as_str(),
        }
        .to_owned()
    }
}

#[derive(AsRefStr, Display, EnumString)]
pub enum TagValue {
    #[strum(serialize = "directory")]
    Directory,
    #[strum(serialize = "favorited")]
    Favorited,
    #[strum(serialize = "file")]
    File,
    #[strum(serialize = "image")]
    Image,
    #[strum(serialize = "symlink")]
    Symlink,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "tags")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub label: String,
    pub value: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

impl Model {
    pub fn tag_pair(&self) -> TagPair {
        (string_to_tag_type(self.label.as_str()), self.value.clone())
    }
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    CrawlQueue,
    IndexedDocument,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::CrawlQueue => Entity::has_many(crawl_queue::Entity).into(),
            Self::IndexedDocument => Entity::has_many(indexed_document::Entity).into(),
        }
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    // Triggered before insert / update
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert {
            self.created_at = Set(chrono::Utc::now());
            self.updated_at = Set(chrono::Utc::now());
        } else {
            self.updated_at = Set(chrono::Utc::now());
        }

        Ok(self)
    }
}

impl Related<super::crawl_queue::Entity> for Entity {
    // The final relation is IndexedDocument -> DocumentTag -> Tag
    fn to() -> RelationDef {
        super::crawl_tag::Relation::Tag.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::crawl_tag::Relation::Tag.def().rev())
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
        label: Set(label.to_string()),
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
        .await?;

    let tag = Entity::find()
        .filter(Column::Label.eq(label.to_string()))
        .filter(Column::Value.eq(value))
        .one(db)
        .await;

    match tag {
        Ok(Some(model)) => Ok(model),
        Err(err) => Err(err),
        _ => Err(DbErr::RecordNotFound(format!(
            "label: {label:?}, value: {value}"
        ))),
    }
}

/// Helper method used to get the database models for the associated tag pairs. If the tag
/// pair does not exist they are created.
pub async fn get_or_create_many<C>(db: &C, tags: &Vec<TagPair>) -> Result<Vec<Model>, DbErr>
where
    C: ConnectionTrait,
{
    let tag_models = tags
        .iter()
        .map(|(label, value)| ActiveModel {
            label: Set(label.to_string()),
            value: Set(value.to_string()),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..Default::default()
        })
        .collect::<Vec<ActiveModel>>();

    let _ = Entity::insert_many(tag_models)
        .on_conflict(
            sea_orm::sea_query::OnConflict::columns(vec![Column::Label, Column::Value])
                .do_nothing()
                .to_owned(),
        )
        .exec_with_returning(db)
        .await;

    let mut condition = Condition::any();
    for (label, value) in tags {
        condition = condition.add(
            Condition::all()
                .add(Column::Label.eq(label.to_string()))
                .add(Column::Value.eq(value.clone())),
        );
    }
    let db_tags = Entity::find().filter(condition).all(db).await;

    match db_tags {
        Ok(models) => Ok(models),
        Err(err) => Err(err),
    }
}

/// Helper method used to get the database models for the associated tag pairs. If the tag
/// pair does not exist they are created. This method uses a pair of strings instead
/// of the TagType enum
pub async fn get_or_create_many_string<C>(
    db: &C,
    tags: &Vec<(String, String)>,
) -> Result<Vec<Model>, DbErr>
where
    C: ConnectionTrait,
{
    let tag_models = tags
        .iter()
        .map(|(label, value)| ActiveModel {
            label: Set(label.clone()),
            value: Set(value.to_string()),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..Default::default()
        })
        .collect::<Vec<ActiveModel>>();

    let _ = Entity::insert_many(tag_models)
        .on_conflict(
            sea_orm::sea_query::OnConflict::columns(vec![Column::Label, Column::Value])
                .do_nothing()
                .to_owned(),
        )
        .exec(db)
        .await;

    let mut condition = Condition::any();
    for (label, value) in tags {
        condition = condition.add(
            Condition::all()
                .add(Column::Label.eq(label.clone()))
                .add(Column::Value.eq(value.clone())),
        );
    }
    let db_tags = Entity::find().filter(condition).all(db).await;

    match db_tags {
        Ok(models) => Ok(models),
        Err(err) => Err(err),
    }
}

/// Helper method used to access the database tag definitions based on the label
/// and value.
pub async fn get_tags_by_value(
    db: &DatabaseConnection,
    tags: &Vec<(String, String)>,
) -> Result<Vec<Model>, DbErr> {
    let mut find = Entity::find();
    for (label, value) in tags {
        find = find
            .filter(Column::Label.eq(label.clone()))
            .filter(Column::Value.eq(value.clone()));
    }

    find.all(db).await
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
            label: Set(tag::TagType::Source.to_string()),
            value: Set("web".to_string()),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..Default::default()
        };

        let mime_tag = tag::ActiveModel {
            label: Set(tag::TagType::MimeType.to_string()),
            value: Set("text/html".to_string()),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..Default::default()
        };

        let conflict = tag::ActiveModel {
            label: Set(tag::TagType::MimeType.to_string()),
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
