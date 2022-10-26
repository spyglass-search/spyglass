use chrono::Duration;
use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "connections")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub access_token: String,
    pub refresh_token: String,
    // Number of seconds til the access token is expired.
    pub expires_in: i64,
    // When the access token was granted (updated on refresh)
    pub granted_at: DateTimeUtc,
    // When this connection was created/updated
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No RelationDef")
    }
}

impl ActiveModelBehavior for ActiveModel {
    // Triggered before insert / update
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        if !insert {
            self.updated_at = Set(chrono::Utc::now());
        }

        Ok(self)
    }
}

impl ActiveModel {
    pub fn new(
        name: String,
        access_token: String,
        refresh_token: String,
        expires_in: Duration,
    ) -> Self {
        Self {
            name: Set(name),
            access_token: Set(access_token),
            refresh_token: Set(refresh_token),
            expires_in: Set(expires_in.num_seconds()),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }
}
