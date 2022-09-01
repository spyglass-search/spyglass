use sea_orm::entity::prelude::*;
use sea_orm::sea_query;
use sea_orm::Set;
use serde::Serialize;
use shared::config::LensConfig;
use std::fmt;

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

impl fmt::Display for LensType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LensType::Simple => write!(f, "Simple"),
            LensType::Plugin => write!(f, "Plugin"),
        }
    }
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
        .filter(Column::LensType.contains(&LensType::Simple.to_string()))
        .exec(db)
        .await?;

    Ok(())
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

    // If it already exists & is not a plugin, simply enable it.
    if let Some(existing) = exists {
        log::info!("updating lens: {}", lens.name);

        let mut updated: ActiveModel = existing.clone().into();
        // Update description / etc.
        updated.author = Set(lens.author.to_string());
        updated.version = Set(lens.version.to_string());
        if lens.trigger.is_empty() {
            updated.trigger = Set(Some(lens.name.clone()));
        } else {
            updated.trigger = Set(Some(lens.trigger.to_string()));
        }
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
    let new_lens = ActiveModel {
        name: Set(lens.name.to_owned()),
        author: Set(lens.author.to_owned()),
        description: Set(lens.description.clone()),
        version: Set(lens.version.to_owned()),
        // NOTE: Only automatically enable simple lenses
        is_enabled: Set(lens_type == LensType::Simple),
        trigger: Set(Some(lens.name.to_owned())),
        lens_type: Set(lens_type),
        ..Default::default()
    };
    new_lens.insert(db).await?;

    Ok(true)
}
