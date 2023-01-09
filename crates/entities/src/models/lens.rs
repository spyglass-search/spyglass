use sea_orm::entity::prelude::*;
use sea_orm::sea_query;
use sea_orm::Set;
use serde::Serialize;
use shared::config::LensConfig;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(1))")]
pub enum LensType {
    // A simple lens with URLs & rules that acts as a "filter"
    #[sea_orm(string_value = "Simple")]
    Simple,
    // A plugin based lens where queueing & rules are dynamic given whatever the
    // source is.
    #[sea_orm(string_value = "Plugin")]
    Plugin,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "lens")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    // Will definitely run into namespace issues later on, something to think about.
    #[sea_orm(unique)]
    pub name: String,
    pub author: String,
    pub description: Option<String>,
    pub version: String,
    // Has this lens been disabled?
    pub is_enabled: bool,
    // Whether this is a text-based or plugin based lens.
    pub lens_type: LensType,
    // Trigger doesn't have to be unique, we can have multiple lenses contributing to
    // the same trigger. Can also be user updatable.
    pub trigger: Option<String>,
    // Indicates the last time the cache was updated
    pub last_cache_update: Option<DateTimeUtc>,
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
            ..ActiveModelTrait::default()
        }
    }
}

pub async fn reset(db: &DatabaseConnection) -> anyhow::Result<()> {
    Entity::update_many()
        .col_expr(Column::IsEnabled, sea_query::Expr::value(false))
        .filter(Column::LensType.eq(LensType::Simple))
        .exec(db)
        .await?;

    Ok(())
}

// Finds the lens using the lens name
pub async fn find_by_name(
    lens_name: &str,
    db: &DatabaseConnection,
) -> Result<Option<Model>, sea_orm::DbErr> {
    Entity::find()
        .filter(Column::Name.eq(lens_name.to_owned()))
        .one(db)
        .await
}

// Updates the lens row in the database with the new cache time
pub async fn update_cache_time(
    lens_name: &String,
    date: DateTimeUtc,
    db: &DatabaseConnection,
) -> anyhow::Result<bool> {
    let exists = Entity::find()
        .filter(Column::Name.eq(lens_name.clone()))
        .one(db)
        .await?;

    if let Some(existing) = exists {
        log::debug!("Updating lens: {} with new cache date {}", lens_name, date);
        let mut updated: ActiveModel = existing.clone().into();
        updated.last_cache_update = Set(Option::Some(date));
        updated.update(db).await?;
        return Ok(true);
    }
    Ok(false)
}

/// True if the lens was added, False if it already exists.
pub async fn add_or_enable(
    db: &DatabaseConnection,
    lens: &LensConfig,
    lens_type: LensType,
) -> anyhow::Result<bool> {
    let exists = Entity::find()
        .filter(Column::Name.eq(lens.name.to_string()))
        .one(db)
        .await?;

    let trigger_label = if lens.trigger.is_empty() {
        lens.name.clone()
    } else {
        lens.trigger.clone()
    };

    // If it already exists & is not a plugin, simply enable it.
    if let Some(existing) = exists {
        log::info!("updating lens: {}", lens.name);
        let mut updated: ActiveModel = existing.clone().into();
        // Update description / etc.
        updated.author = Set(lens.author.to_string());
        updated.version = Set(lens.version.to_string());
        updated.trigger = Set(Some(trigger_label));
        match &lens.description {
            Some(desc) => updated.description = Set(Some(desc.clone())),
            None => updated.description = Set(None),
        }

        // TODO: This is super hacky, think about a long term way of storing
        // enabled/disabled lenses/plugins etc.
        if lens_type == LensType::Simple {
            updated.is_enabled = Set(true);
        }

        updated.update(db).await?;
        return Ok(false);
    }

    // Otherwise add the lens & enable it.
    log::info!("adding lens: {}", lens.name);
    let new_lens = ActiveModel {
        name: Set(lens.name.to_owned()),
        author: Set(lens.author.to_owned()),
        description: Set(lens.description.clone()),
        version: Set(lens.version.to_owned()),
        // NOTE: Only automatically enable simple lenses
        is_enabled: Set(lens_type == LensType::Simple),
        trigger: Set(Some(trigger_label)),
        lens_type: Set(lens_type),
        ..Default::default()
    };
    new_lens.insert(db).await?;

    Ok(true)
}

#[cfg(test)]
mod test {
    use super::{add_or_enable, Entity};
    use crate::test::setup_test_db;
    use sea_orm::EntityTrait;
    use shared::config::LensConfig;

    #[tokio::test]
    async fn test_add_or_enable() {
        let db = setup_test_db().await;
        let mut lens = LensConfig {
            name: "test_lens".to_owned(),
            trigger: "trigger".to_owned(),
            urls: vec!["https://example.com".to_owned()],
            ..Default::default()
        };

        let is_new = add_or_enable(&db, &lens, super::LensType::Simple)
            .await
            .unwrap();
        assert_eq!(is_new, true);

        // Check that we have the right values.
        let model = Entity::find().one(&db).await.unwrap().unwrap();
        assert_eq!(model.name, "test_lens".to_owned());
        assert_eq!(model.trigger, Some("trigger".to_owned()));
        assert_eq!(model.description, None);

        // Update & trying to insert again should update values
        lens.trigger = "new_trigger".to_owned();
        lens.description = Some("description".to_owned());
        let is_new = add_or_enable(&db, &lens, super::LensType::Simple)
            .await
            .unwrap();
        assert_eq!(is_new, false);

        let model = Entity::find().one(&db).await.unwrap().unwrap();
        assert_eq!(model.name, "test_lens".to_owned());
        assert_eq!(model.trigger, Some("new_trigger".to_owned()));
        assert_eq!(model.description, Some("description".to_owned()));
    }
}
