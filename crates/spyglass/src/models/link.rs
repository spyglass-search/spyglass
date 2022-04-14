use sea_orm::entity::prelude::*;
use sea_orm::Set;
use url::Url;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "link")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub src_domain: String,
    pub src_url: String,
    pub dst_domain: String,
    pub dst_url: String,
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

pub type LinkSaved = bool;
pub async fn save_link(db: &DatabaseConnection, src: &String, dst: &String) -> anyhow::Result<LinkSaved, sea_orm::DbErr> {
    let src_url = Url::parse(src).unwrap();
    let dst_url = Url::parse(dst).unwrap();

    // Ignore links in the same domain
    if src_url.host_str().unwrap() == dst_url.host_str().unwrap() {
        return Ok(false);
    }

    let new_link = ActiveModel {
        src_domain: Set(src_url.host_str().unwrap().to_owned()),
        src_url: Set(src.to_owned()),
        dst_domain: Set(dst_url.host_str().unwrap().to_owned()),
        dst_url: Set(dst.to_owned()),
        ..Default::default()
    };

    new_link.insert(db).await?;
    Ok(true)
}