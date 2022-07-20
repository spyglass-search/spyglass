use sea_orm::entity::prelude::*;
use sea_orm::sea_query;
use sea_orm::Set;
use serde::Serialize;
use shared::config::Lens;
use std::fmt;

#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(1))")]
pub enum LensType {
    // A simple lens with URLs & rules
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

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize)]
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
    name: &str,
    author: &str,
    description: Option<&String>,
    version: &str,
    lens_type: LensType,
) -> anyhow::Result<bool> {
    let exists = Entity::find()
        .filter(Column::Name.eq(name.to_string()))
        .one(db)
        .await?;

    // If it already exists & is not a plugin, simply enable it.
    if let Some(existing) = exists {
        // TODO: This is super hacky, think about a long term way of storing
        // enabled/disabled lenses/plugins etc.
        if lens_type == LensType::Simple {
            let mut updated: ActiveModel = existing.clone().into();
            updated.is_enabled = Set(true);
            updated.update(db).await?;
            return Ok(false);
        }
    }

    // Otherwise add the lens & enable it.
    let new_lens = ActiveModel {
        name: Set(name.to_owned()),
        author: Set(author.to_owned()),
        description: Set(description.map(String::from)),
        version: Set(version.to_owned()),
        is_enabled: Set(true),
        trigger: Set(Some(name.to_owned())),
        lens_type: Set(lens_type),
        ..Default::default()
    };
    new_lens.insert(db).await?;

    Ok(true)
}
