use sea_orm::sea_query::OnConflict;
use sea_orm::{prelude::*, FromQueryResult, InsertResult, Statement};
use sea_orm::{ActiveModelBehavior, DbErr, EntityTrait, Set};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Eq)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum QueueStatus {
    #[sea_orm(string_value = "Queued")]
    Queued,
    #[sea_orm(string_value = "Processing")]
    Processing,
    #[sea_orm(string_value = "Completed")]
    Completed,
    #[sea_orm(string_value = "Failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "embedding_queue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique, indexed)]
    pub document_id: String,
    pub content: Option<String>,
    pub status: QueueStatus,
    pub errors: Option<String>,
    pub indexed_document_id: i64,
    /// When this was first added to the crawl queue.
    pub created_at: DateTimeUtc,
    /// When this task was last updated.
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    IndexedDocument,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::IndexedDocument => Entity::belongs_to(super::indexed_document::Entity)
                .from(Column::IndexedDocumentId)
                .to(super::indexed_document::Column::Id)
                .into(),
        }
    }
}

impl Related<super::indexed_document::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::IndexedDocument.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            status: Set(QueueStatus::Queued),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }
}

pub async fn enqueue<C>(
    db: &C,
    document_id: &str,
    indexed_document_id: i64,
    content: &str,
) -> Result<InsertResult<ActiveModel>, DbErr>
where
    C: ConnectionTrait,
{
    let mut model = ActiveModel::new();
    model.document_id = Set(document_id.to_string());
    model.indexed_document_id = Set(indexed_document_id);
    model.content = Set(Some(content.to_string()));

    Entity::insert(model)
        .on_conflict(
            OnConflict::column(Column::DocumentId)
                .update_columns([Column::Status, Column::Content])
                .to_owned(),
        )
        .exec(db)
        .await
}

pub async fn add_to_queue<C>(db: &C, to_add: &[ActiveModel]) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    Entity::insert_many(to_add.to_vec())
        .on_conflict(
            OnConflict::column(Column::DocumentId)
                .update_columns([Column::Status, Column::Content])
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
    Ok(())
}

#[derive(Clone, Debug, FromQueryResult)]
pub struct Job {
    pub id: i64,
}

pub async fn check_for_embedding_jobs(db: &DatabaseConnection) -> Result<Option<Job>, DbErr> {
    let count = Entity::find()
        .filter(Column::Status.eq(QueueStatus::Processing))
        .count(db)
        .await?;

    if count >= 3 {
        log::debug!("Waiting for previous embedding tasks to finish");
        return Ok(None);
    }

    let query = Statement::from_string(
        db.get_database_backend(),
        r#"
           UPDATE embedding_queue AS eq
        SET
            status = 'Processing',
            updated_at = DATETIME('now')
        WHERE id IN (
            SELECT
                id
            FROM embedding_queue
            WHERE status = 'Queued'
            ORDER By created_at
            LIMIT 1
        )
        RETURNING id"#
            .to_string(),
    );

    Job::find_by_statement(query).one(db).await
}

pub async fn mark_done(db: &DatabaseConnection, id: i64) {
    if let Ok(Some(embedding)) = Entity::find_by_id(id).one(db).await {
        let mut updated: ActiveModel = embedding.clone().into();
        updated.status = Set(QueueStatus::Completed);
        updated.content = Set(None);
        updated.errors = Set(None);
        let _ = updated.update(db).await;
    }
}

pub async fn mark_failed(db: &DatabaseConnection, id: i64, error: Option<String>) {
    if let Ok(Some(embedding)) = Entity::find_by_id(id).one(db).await {
        let mut updated: ActiveModel = embedding.clone().into();
        updated.status = Set(QueueStatus::Failed);
        updated.content = Set(None);
        updated.errors = Set(error);
        let _ = updated.update(db).await;
    }
}
