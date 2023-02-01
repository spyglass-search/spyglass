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
    #[sea_orm(primary_key)]
    pub id: i64,
    // Connection ID
    pub api_id: String,
    // Account email/id associated w/ this credential.
    pub account: String,
    // access/refresh token used for authentication.
    pub access_token: String,
    pub refresh_token: Option<String>,
    // Authorized scopes for this token
    pub scopes: Scopes,
    // Number of seconds til the access token is expired.
    // NULL if it never expires.
    pub expires_in: Option<i64>,
    // When the access token was granted (updated on refresh)
    pub granted_at: DateTimeUtc,
    // Whether or not this connection is currently syncing.
    pub is_syncing: bool,
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
        account: String,
        access_token: String,
        refresh_token: Option<String>,
        expires_in: Option<i64>,
        scopes: Vec<String>,
    ) -> Self {
        Self {
            api_id: Set(id),
            account: Set(account),
            access_token: Set(access_token),
            refresh_token: Set(refresh_token),
            scopes: Set(Scopes { scopes }),
            expires_in: Set(expires_in),
            granted_at: Set(chrono::Utc::now()),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..Default::default()
        }
    }
}

/// Helper method used to access all configured connections
pub async fn get_all_connections(db: &DatabaseConnection) -> Vec<Model> {
    Entity::find().all(db).await.unwrap_or_default()
}

/// Helper method used to get the entry for the specified id and account
pub async fn get_by_id(
    db: &DatabaseConnection,
    id: &str,
    account: &str,
) -> Result<Option<Model>, sea_orm::DbErr> {
    Entity::find()
        .filter(Column::ApiId.eq(id))
        .filter(Column::Account.eq(account))
        .one(db)
        .await
}

pub async fn set_sync_status(
    db: &DatabaseConnection,
    id: &str,
    account: &str,
    is_syncing: bool,
) -> Result<(), sea_orm::DbErr> {
    if let Some(model) = get_by_id(db, id, account).await? {
        let mut update: ActiveModel = model.into();
        update.is_syncing = Set(is_syncing);
        update.save(db).await?;
    }

    Ok(())
}
