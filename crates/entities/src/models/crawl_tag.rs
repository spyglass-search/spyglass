use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "crawl_tag")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub crawl_queue_id: i64,
    pub tag_id: i64,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    CrawlQueue,
    Tag,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::CrawlQueue => Entity::belongs_to(super::crawl_queue::Entity)
                .from(Column::CrawlQueueId)
                .to(super::crawl_queue::Column::Id)
                .into(),
            Self::Tag => Entity::belongs_to(super::tag::Entity)
                .from(Column::TagId)
                .to(super::tag::Column::Id)
                .into(),
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
