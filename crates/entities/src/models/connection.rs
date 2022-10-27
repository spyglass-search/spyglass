use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct Scopes {
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "connections")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    // access/refresh token used for authentication.
    pub access_token: String,
    pub refresh_token: String,
    // Authorized scopes for this token
    pub scopes: Scopes,
    // Number of seconds til the access token is expired.
    // NULL if it never expires.
    pub expires_in: Option<i64>,
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
        id: String,
        access_token: String,
        refresh_token: String,
        expires_in: Option<i64>,
        scopes: Vec<String>,
    ) -> Self {
        Self {
            id: Set(id),
            access_token: Set(access_token),
            refresh_token: Set(refresh_token),
            scopes: Set(Scopes { scopes }),
            expires_in: Set(expires_in),
            granted_at: Set(chrono::Utc::now()),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }
}
