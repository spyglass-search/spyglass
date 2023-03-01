use sea_orm::entity::prelude::*;
use sea_orm::{QueryOrder, QuerySelect, Set};
use serde::{Deserialize, Serialize};

use super::{crawl_queue, indexed_document};

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
    /// When this connection was created
    pub created_at: DateTimeUtc,
    /// When this connection was last synced
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No RelationDef")
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    // Triggered before insert / update
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
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

/// Finds the oldest connection that hasn't been synced & sync it!
pub async fn dequeue_sync(db: &DatabaseConnection) -> Option<Model> {
    let model = Entity::find().order_by_asc(Column::UpdatedAt).one(db).await;

    if let Ok(Some(task)) = model {
        let now = chrono::Utc::now();
        let last_synced = now - task.updated_at;
        if last_synced.num_hours() < 24 {
            return None;
        }

        // Set to synicng & update the updated at timestamp.
        let mut update: ActiveModel = task.clone().into();
        update.is_syncing = Set(true);
        update.updated_at = Set(now);
        let _ = update.save(db).await;
        Some(task)
    } else {
        None
    }
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

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
enum QueryAs {
    Id,
}

/// Removes relevant information from DB, but DOES NOT DELETE FROM SEARCH INDEX
pub async fn revoke_connection(
    db: &DatabaseConnection,
    api_id: &str,
    account: &str,
) -> Result<(), DbErr> {
    let url_like = format!("api://{account}@{api_id}%");

    // Remove from connections list
    let _ = Entity::delete_many()
        .filter(Column::ApiId.eq(api_id))
        .filter(Column::Account.eq(account))
        .exec(db)
        .await;

    // Remove any crawl queue items
    let cqids = crawl_queue::Entity::find()
        .column_as(crawl_queue::Column::Id, QueryAs::Id)
        .filter(crawl_queue::Column::Domain.eq(api_id))
        .filter(crawl_queue::Column::Url.like(&url_like))
        .into_values::<_, QueryAs>()
        .all(db)
        .await
        .unwrap_or_default();
    crawl_queue::delete_many_by_id(db, &cqids).await?;

    // Remove from indexed_docs
    let dbids: Vec<i64> = indexed_document::Entity::find()
        .column_as(indexed_document::Column::Id, QueryAs::Id)
        .filter(indexed_document::Column::Domain.eq(api_id))
        .filter(indexed_document::Column::Url.like(&url_like))
        .into_values::<_, QueryAs>()
        .all(db)
        .await
        .unwrap_or_default();
    indexed_document::delete_many_by_id(db, &dbids).await?;

    Ok(())
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
        update.updated_at = Set(chrono::Utc::now());
        update.save(db).await?;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::ActiveModel;
    use crate::test::setup_test_db;
    use chrono::{TimeZone, Utc};
    use sea_orm::{ActiveModelTrait, Set};

    /// Should always dequeue the oldest one first.
    #[tokio::test]
    async fn test_dequeue_sync() {
        let db = setup_test_db().await;

        let newer = Utc.with_ymd_and_hms(2023, 2, 2, 0, 0, 0).unwrap();
        let older = Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap();

        let one = ActiveModel {
            api_id: Set("test_one".into()),
            updated_at: Set(newer),
            ..Default::default()
        };
        let _ = one.insert(&db).await.expect("Unable to insert");

        let two = ActiveModel {
            api_id: Set("test_two".into()),
            updated_at: Set(older),
            ..Default::default()
        };
        let two = two.insert(&db).await.expect("Unable to insert");

        let result = super::dequeue_sync(&db).await.expect("Should be a result");

        assert_eq!(result.api_id, two.api_id);
    }
}
