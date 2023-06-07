use regex::{Regex, RegexBuilder, RegexSet, RegexSetBuilder};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{OnConflict, Query, SqliteQueryBuilder};
use sea_orm::{
    sea_query, ConnectionTrait, FromQueryResult, InsertResult, QueryTrait, Set, Statement,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use url::Url;

use super::crawl_tag;
use super::indexed_document;
use super::tag::{self, get_or_create, TagPair};
use crate::BATCH_SIZE;
use shared::config::{LensConfig, LensRule, Limit, UrlSanitizeConfig, UserSettings};
use shared::regex::{regex_for_domain, regex_for_prefix};

const MAX_RETRIES: u8 = 5;

#[derive(Debug, Error)]
pub enum EnqueueError {
    #[error("Database error: {0}")]
    DbError(#[from] sea_orm::DbErr),
    #[error("other enqueue error: {0}")]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, Eq)]
#[sea_orm(rs_type = "String", db_type = "String(None)")]
pub enum TaskErrorType {
    #[sea_orm(string_value = "Collect")]
    Collect,
    #[sea_orm(string_value = "Duplicate")]
    Duplicate,
    #[sea_orm(string_value = "Fetch")]
    Fetch,
    #[sea_orm(string_value = "Parse")]
    Parse,
    #[sea_orm(string_value = "Tag")]
    Tag,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct TaskError {
    error_type: TaskErrorType,
    msg: String,
}

#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Eq)]
#[sea_orm(rs_type = "String", db_type = "String(None)")]
pub enum CrawlStatus {
    #[sea_orm(string_value = "Initial")]
    Initial,
    #[sea_orm(string_value = "Queued")]
    Queued,
    #[sea_orm(string_value = "Processing")]
    Processing,
    #[sea_orm(string_value = "Completed")]
    Completed,
    #[sea_orm(string_value = "Failed")]
    Failed,
}

#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Eq, Default)]
#[sea_orm(rs_type = "String", db_type = "String(None)")]
pub enum CrawlType {
    #[sea_orm(string_value = "API")]
    Api,
    #[sea_orm(string_value = "Bootstrap")]
    Bootstrap,
    #[sea_orm(string_value = "Normal")]
    #[default]
    Normal,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "crawl_queue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Domain/host of the URL to be crawled
    pub domain: String,
    /// URL to crawl
    #[sea_orm(unique)]
    pub url: String,
    /// Task status.
    pub status: CrawlStatus,
    /// If this failed, the reason for the failure
    pub error: Option<TaskError>,
    /// Data that we want to keep around about this task.
    pub data: Option<String>,
    /// Number of retries for this task.
    #[sea_orm(default_value = 0)]
    pub num_retries: u8,
    /// Crawl Type
    pub crawl_type: CrawlType,
    /// When this was first added to the crawl queue.
    pub created_at: DateTimeUtc,
    /// When this task was last updated.
    pub updated_at: DateTimeUtc,
    pub pipeline: Option<String>,
}

impl Related<super::tag::Entity> for Entity {
    // The final relation is IndexedDocument -> DocumentTag -> Tag
    fn to() -> RelationDef {
        super::crawl_tag::Relation::Tag.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::crawl_tag::Relation::CrawlQueue.def().rev())
    }
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    Tag,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::Tag => Entity::has_many(tag::Entity).into(),
        }
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            crawl_type: Set(CrawlType::Normal),
            status: Set(CrawlStatus::Queued),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }

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

impl Model {
    pub async fn insert_tags<C: ConnectionTrait>(
        &self,
        db: &C,
        tags: &[TagPair],
    ) -> Result<InsertResult<crawl_tag::ActiveModel>, DbErr> {
        let mut tag_models: Vec<tag::Model> = Vec::new();
        for (label, value) in tags.iter() {
            match get_or_create(db, label.to_owned(), value).await {
                Ok(tag) => tag_models.push(tag),
                Err(err) => log::error!("{}", err),
            }
        }

        // create connections for each tag
        let doc_tags = tag_models
            .iter()
            .map(|t| crawl_tag::ActiveModel {
                crawl_queue_id: Set(self.id),
                tag_id: Set(t.id),
                created_at: Set(chrono::Utc::now()),
                updated_at: Set(chrono::Utc::now()),
                ..Default::default()
            })
            .collect::<Vec<crawl_tag::ActiveModel>>();

        // Insert connections, ignoring duplicates
        crawl_tag::Entity::insert_many(doc_tags)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns(vec![
                    crawl_tag::Column::CrawlQueueId,
                    crawl_tag::Column::TagId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(db)
            .await
    }
}

pub async fn reset_processing(db: &DatabaseConnection) -> anyhow::Result<()> {
    Entity::update_many()
        .col_expr(Column::Status, sea_query::Expr::value(CrawlStatus::Queued))
        .filter(Column::Status.eq(CrawlStatus::Processing))
        .exec(db)
        .await?;

    Ok(())
}

pub async fn num_queued(
    db: &DatabaseConnection,
    status: CrawlStatus,
) -> anyhow::Result<u64, sea_orm::DbErr> {
    let res = Entity::find()
        .filter(Column::Status.eq(status))
        .count(db)
        .await?;

    Ok(res)
}

fn gen_dequeue_sql(db: &DatabaseConnection, user_settings: &UserSettings) -> Statement {
    Statement::from_sql_and_values(
        db.get_database_backend(),
        include_str!("sql/dequeue.sqlx"),
        vec![
            user_settings.domain_crawl_limit.value().into(),
            user_settings.inflight_domain_limit.value().into(),
        ],
    )
}
struct LensRuleSets {
    // Allow if any URLs match
    allow_list: Vec<String>,
    // Skip if any URLs match
    skip_list: Vec<String>,
    // Skip if any URLs do not match
    restrict_list: Vec<String>,
}

/// Create a set of allow/skip rules from a Lens
fn create_ruleset_from_lens(lens: &LensConfig) -> LensRuleSets {
    let mut allow_list = Vec::new();
    let mut skip_list: Vec<String> = Vec::new();
    let mut restrict_list: Vec<String> = Vec::new();

    // Build regex from domain
    for domain in lens.domains.iter() {
        allow_list.push(regex_for_domain(domain));
    }

    // Build regex from url rules
    for prefix in lens.urls.iter() {
        allow_list.push(regex_for_prefix(prefix));
    }

    // Build regex from rules
    for rule in lens.rules.iter() {
        match rule {
            LensRule::SkipURL(_) => {
                skip_list.push(rule.to_regex());
            }
            LensRule::LimitURLDepth(_, _) => {
                restrict_list.push(rule.to_regex());
            }
            LensRule::SanitizeUrls(_, _) => {}
        }
    }

    LensRuleSets {
        allow_list,
        skip_list,
        restrict_list,
    }
}

/// How many tasks do we have in progress?
pub async fn num_tasks_in_progress(db: &DatabaseConnection) -> anyhow::Result<u64, DbErr> {
    Entity::find()
        .filter(Column::Status.eq(CrawlStatus::Processing))
        .count(db)
        .await
}

/// How many tasks do we have in progress?
pub async fn num_of_files_in_progress(db: &DatabaseConnection) -> anyhow::Result<u64, DbErr> {
    Entity::find()
        .filter(Column::Status.eq(CrawlStatus::Processing))
        .count(db)
        .await
}

/// Get the next url in the crawl queue
pub async fn dequeue(
    db: &DatabaseConnection,
    user_settings: &UserSettings,
) -> anyhow::Result<Option<Model>, sea_orm::DbErr> {
    // Check for inflight limits
    if let Limit::Finite(inflight_crawl_limit) = user_settings.inflight_crawl_limit {
        // How many do we have in progress?
        let num_in_progress = num_tasks_in_progress(db).await?;
        // Nothing to do if we have too many crawls
        if num_in_progress >= inflight_crawl_limit as u64 {
            return Ok(None);
        }
    }

    // Prioritize any bootstrapping tasks first.
    let entity = {
        let result = Entity::find()
            .filter(Column::CrawlType.eq(CrawlType::Bootstrap))
            .filter(Column::Status.eq(CrawlStatus::Queued))
            .one(db)
            .await?;

        if let Some(task) = result {
            Some(task)
        } else {
            // Otherwise, grab a URL off the stack & send it back.
            Entity::find()
                .from_raw_sql(gen_dequeue_sql(db, user_settings))
                .one(db)
                .await?
        }
    };

    // Grab new entity and immediately mark in-progress
    if let Some(task) = entity {
        let mut update: ActiveModel = task.into();
        update.status = Set(CrawlStatus::Processing);
        return match update.update(db).await {
            Ok(model) => Ok(Some(model)),
            // Deleted while being processed?
            Err(err) => {
                log::error!("Unable to update crawl task: {}", err);
                Ok(None)
            }
        };
    }

    Ok(None)
}

/// Get the next url in the crawl queue
pub async fn dequeue_files(
    db: &DatabaseConnection,
    user_settings: &UserSettings,
) -> anyhow::Result<Option<Model>, sea_orm::DbErr> {
    // Check for inflight limits
    if let Limit::Finite(inflight_crawl_limit) = user_settings.inflight_crawl_limit {
        // How many do we have in progress?
        let num_in_progress = num_of_files_in_progress(db).await?;
        // Nothing to do if we have too many crawls
        if num_in_progress >= inflight_crawl_limit as u64 {
            return Ok(None);
        }
    }

    let entity = Entity::find()
        .filter(Column::Status.eq(CrawlStatus::Queued))
        .filter(Column::Url.starts_with("file:"))
        .one(db)
        .await?;

    // Grab new entity and immediately mark in-progress
    if let Some(task) = entity {
        let mut update: ActiveModel = task.into();
        update.status = Set(CrawlStatus::Processing);
        return match update.update(db).await {
            Ok(model) => Ok(Some(model)),
            // Deleted while being processed?
            Err(err) => {
                log::error!("Unable to update crawl task: {}", err);
                Ok(None)
            }
        };
    }

    Ok(None)
}

/// Add url to the crawl queue
#[derive(PartialEq, Eq)]
pub enum SkipReason {
    Invalid,
    Blocked,
    Duplicate,
}

#[derive(Default)]
pub struct EnqueueSettings {
    pub crawl_type: CrawlType,
    pub tags: Vec<TagPair>,
    pub force_allow: bool,
    pub is_recrawl: bool,
}

fn url_is_allowed(
    url: &str,
    allow_list: &RegexSet,
    restrict_list: &RegexSet,
    skip_list: &RegexSet,
) -> bool {
    // Ignore domains on blacklist
    if skip_list.is_match(url)
    // Skip if any URLs do not match this restriction
    || (!restrict_list.is_empty()
        && !restrict_list.is_match(url))
    {
        return false;
    }

    // If external links are not allowed, only allow crawls specified
    // in our lenses
    !allow_list.is_empty() && allow_list.is_match(url)
}

fn filter_urls(
    lenses: &[LensConfig],
    settings: &UserSettings,
    overrides: &EnqueueSettings,
    urls: &[String],
) -> anyhow::Result<Vec<String>> {
    let mut allow_list: Vec<String> = Vec::new();
    let mut skip_list: Vec<String> = Vec::new();
    let mut restrict_list: Vec<String> = Vec::new();
    let mut sanitize_rules: Vec<(Regex, UrlSanitizeConfig)> = Vec::new();

    for domain in settings.block_list.iter() {
        skip_list.push(regex_for_domain(domain));
    }

    for lens in lenses {
        let ruleset = create_ruleset_from_lens(lens);
        allow_list.extend(ruleset.allow_list);
        skip_list.extend(ruleset.skip_list);
        restrict_list.extend(ruleset.restrict_list);

        sanitize_rules.extend(lens.rules.iter().filter_map(|rule| {
            if let LensRule::SanitizeUrls(_, config) = rule {
                let regex = RegexBuilder::new(&rule.to_regex()).build().ok()?;
                Some((regex, config.clone()))
            } else {
                None
            }
        }));
    }

    let allow_list = RegexSetBuilder::new(allow_list)
        .size_limit(100_000_000)
        .build()?;
    let skip_list = RegexSetBuilder::new(skip_list)
        .size_limit(100_000_000)
        .build()?;
    let restrict_list = RegexSetBuilder::new(restrict_list)
        .size_limit(100_000_000)
        .build()?;

    // Ignore invalid URLs
    let res = urls
        .iter()
        // Only look at valid URLs
        .flat_map(|s| Url::parse(s))
        .filter_map(|url| {
            // Check that we can handle this scheme
            if url.scheme() != "http"
                && url.scheme() != "https"
                && url.scheme() != "file"
                && url.scheme() != "api"
            {
                None
            } else {
                Some(url)
            }
        })
        .filter_map(|mut url| {
            if overrides.force_allow {
                return Some(url.to_string());
            }

            // Always ignore fragments, otherwise crawling
            // https://wikipedia.org/Rust#Blah would be considered different than
            // https://wikipedia.org/Rust
            url.set_fragment(None);

            for (regex, config) in &sanitize_rules {
                if regex.is_match(url.as_str()) {
                    sanitize_url(&mut url, config);
                }
            }

            let normalized = url.to_string();
            let no_end_slash = if normalized.ends_with('/') {
                Some(normalized.trim_end_matches('/').to_string())
            } else {
                None
            };

            let mut checks = Vec::new();
            checks.push(url_is_allowed(
                &normalized,
                &allow_list,
                &restrict_list,
                &skip_list,
            ));
            if let Some(no_end_slash) = no_end_slash {
                checks.push(url_is_allowed(
                    &no_end_slash,
                    &allow_list,
                    &restrict_list,
                    &skip_list,
                ));
            }

            if checks.iter().any(|f| *f) {
                Some(normalized)
            } else {
                None
            }
        })
        .collect::<Vec<String>>();

    Ok(res)
}

pub async fn enqueue_local_files(
    db: &DatabaseConnection,
    urls: &[String],
    overrides: &EnqueueSettings,
    pipeline: Option<String>,
) -> anyhow::Result<(), sea_orm::DbErr> {
    for chunk in urls.chunks(BATCH_SIZE) {
        let model = chunk
            .iter()
            .map(|url| ActiveModel {
                domain: Set(String::from("localhost")),
                crawl_type: Set(overrides.crawl_type.clone()),
                status: Set(CrawlStatus::Initial),
                url: Set(url.to_string()),
                pipeline: Set(pipeline.clone()),
                ..Default::default()
            })
            .collect::<Vec<ActiveModel>>();

        let on_conflict = if overrides.is_recrawl {
            OnConflict::column(Column::Url)
                .update_column(Column::Status)
                .to_owned()
        } else {
            OnConflict::column(Column::Url).do_nothing().to_owned()
        };

        let _insert = Entity::insert_many(model)
            .on_conflict(on_conflict)
            .exec(db)
            .await?;
        let inserted_rows = Entity::find()
            .filter(Column::Url.is_in(chunk.to_vec()))
            .all(db)
            .await?;

        let ids = inserted_rows.iter().map(|row| row.id).collect::<Vec<i64>>();
        let tag_rslt = insert_tags_many(db, &inserted_rows, &overrides.tags).await;
        if tag_rslt.is_ok() {
            let query = Query::update()
                .table(Entity.table_ref())
                .values([(Column::Status, CrawlStatus::Queued.into())])
                .and_where(Column::Id.is_in(ids))
                .to_owned();

            let query = query.to_string(SqliteQueryBuilder);
            db.execute(Statement::from_string(db.get_database_backend(), query))
                .await?;
        }
    }
    Ok(())
}

pub async fn enqueue_all<C: ConnectionTrait>(
    db: &C,
    urls: &[String],
    lenses: &[LensConfig],
    settings: &UserSettings,
    overrides: &EnqueueSettings,
    pipeline: Option<String>,
) -> anyhow::Result<(), EnqueueError> {
    // Filter URLs
    let urls = filter_urls(lenses, settings, overrides, urls).unwrap_or_default();

    // Ignore urls already indexed
    let mut is_indexed: HashSet<String> = HashSet::with_capacity(urls.len());
    if !overrides.is_recrawl {
        for chunk in urls.chunks(BATCH_SIZE) {
            let chunk = chunk.iter().map(|url| url.to_string()).collect::<Vec<_>>();
            for entry in indexed_document::Entity::find()
                .filter(indexed_document::Column::Url.is_in(chunk.clone()))
                .all(db)
                .await?
                .iter()
            {
                is_indexed.insert(entry.url.to_string());
            }
        }
    }

    let to_add: Vec<ActiveModel> = urls
        .into_iter()
        .filter_map(|url| {
            let mut result = None;
            if !is_indexed.contains(&url) {
                if let Ok(parsed) = Url::parse(&url) {
                    let domain = match parsed.scheme() {
                        "file" => "localhost",
                        _ => parsed.host_str()?,
                    };

                    result = Some(ActiveModel {
                        domain: Set(domain.to_string()),
                        crawl_type: Set(overrides.crawl_type.clone()),
                        url: Set(url.to_string()),
                        pipeline: Set(pipeline.clone()),
                        ..Default::default()
                    });
                }
            }
            result
        })
        .collect();

    // If we have tags, update the tags for the already indexed URLs
    if !overrides.tags.is_empty() && !is_indexed.is_empty() {
        let to_update = Entity::find()
            .filter(Column::Url.is_in(is_indexed))
            .all(db)
            .await
            .unwrap_or_default();

        if !to_update.is_empty() {
            let result = insert_tags_many(db, &to_update, &overrides.tags).await;
            if let Err(error) = result {
                log::error!("Error updating tags for crawl: {:?}", error);
            }
        }
    }

    if to_add.is_empty() {
        return Ok(());
    }

    let on_conflict = if overrides.is_recrawl {
        OnConflict::column(Column::Url)
            .update_column(Column::Status)
            .to_owned()
    } else {
        OnConflict::column(Column::Url).do_nothing().to_owned()
    };

    for to_add in to_add.chunks(BATCH_SIZE) {
        let owned = to_add.iter().map(|r| r.to_owned()).collect::<Vec<_>>();
        let urls = to_add
            .iter()
            .map(|r| r.url.clone().unwrap())
            .collect::<Vec<String>>();

        let (sql, values) = Entity::insert_many(owned)
            .query()
            .on_conflict(on_conflict.clone())
            .build(SqliteQueryBuilder);

        let values: Vec<Value> = values.iter().map(|x| x.to_owned()).collect();
        let statement = Statement::from_sql_and_values(db.get_database_backend(), &sql, values);
        if let Err(err) = db.execute(statement).await {
            log::warn!("insert_many error: {err}");
        } else if !overrides.tags.is_empty() {
            let inserted_rows = Entity::find()
                .filter(Column::Url.is_in(urls))
                .all(db)
                .await
                .unwrap_or_default();

            if !inserted_rows.is_empty() {
                if let Err(error) = insert_tags_many(db, &inserted_rows, &overrides.tags).await {
                    log::warn!("Error inserting tags for crawl - {:?}", error);
                }
            }
        }
    }

    Ok(())
}

pub async fn mark_done(
    db: &DatabaseConnection,
    id: i64,
    tags: Option<Vec<TagPair>>,
) -> Option<Model> {
    if let Ok(Some(crawl)) = Entity::find_by_id(id).one(db).await {
        if let Some(tags) = tags {
            if !tags.is_empty() {
                let _ = crawl.insert_tags(db, &tags).await;
            }
        }

        let mut updated: ActiveModel = crawl.into();
        updated.status = Set(CrawlStatus::Completed);
        updated.updated_at = Set(chrono::Utc::now());
        updated.update(db).await.ok()
    } else {
        None
    }
}

pub async fn mark_failed(db: &DatabaseConnection, id: i64, retry: bool) {
    if let Ok(Some(crawl)) = Entity::find_by_id(id).one(db).await {
        let mut updated: ActiveModel = crawl.clone().into();

        // Bump up number of retries if this failed
        if retry && crawl.num_retries <= MAX_RETRIES {
            updated.num_retries = Set(crawl.num_retries + 1);
            // Queue again
            updated.status = Set(CrawlStatus::Queued);
        } else {
            updated.status = Set(CrawlStatus::Failed);
        }
        let _ = updated.update(db).await;
    }
}

pub async fn insert_tags_by_id<C: ConnectionTrait>(
    db: &C,
    docs: &[Model],
    tag_ids: &[i64],
) -> Result<InsertResult<crawl_tag::ActiveModel>, DbErr> {
    // create connections for each tag
    let crawl_tags = docs
        .iter()
        .flat_map(|model| {
            tag_ids.iter().map(|t| crawl_tag::ActiveModel {
                crawl_queue_id: Set(model.id),
                tag_id: Set(*t),
                created_at: Set(chrono::Utc::now()),
                updated_at: Set(chrono::Utc::now()),
                ..Default::default()
            })
        })
        .collect::<Vec<crawl_tag::ActiveModel>>();

    // Insert connections, ignoring duplicates
    crawl_tag::Entity::insert_many(crawl_tags)
        .on_conflict(
            sea_orm::sea_query::OnConflict::columns(vec![
                crawl_tag::Column::CrawlQueueId,
                crawl_tag::Column::TagId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec(db)
        .await
}

/// Inserts an entry into the tag table for each crawl and
/// tag pair provided
pub async fn insert_tags_many<C: ConnectionTrait>(
    db: &C,
    docs: &[Model],
    tags: &[TagPair],
) -> Result<(), DbErr> {
    let mut tag_ids: Vec<i64> = Vec::new();
    for (label, value) in tags.iter() {
        match get_or_create(db, label.to_owned(), value).await {
            Ok(tag) => tag_ids.push(tag.id),
            Err(err) => log::warn!("Unable to get/create tag: {err}"),
        }
    }

    let res = insert_tags_by_id(db, docs, &tag_ids).await;
    match res {
        Ok(_) | Err(DbErr::RecordNotInserted) => Ok(()),
        Err(err) => Err(err),
    }
}

/// Remove tasks from the crawl queue that match `rule`. Rule is expected
/// to be a SQL like statement.
pub async fn remove_by_rule(db: &DatabaseConnection, rule: &str) -> anyhow::Result<u64> {
    let dbids: Vec<i64> = Entity::find()
        .filter(Column::Url.like(rule))
        .all(db)
        .await?
        .iter()
        .map(|x| x.id)
        .collect();

    let rows_affected = delete_many_by_id(db, &dbids).await?;
    Ok(rows_affected)
}

/// Update the URL of a task. Typically used after a crawl to set the canonical URL
/// extracted from the crawl result. If there's a conflict, this means another crawl task
/// already points to this same URL and thus can be safely removed.
pub async fn update_or_remove_task(
    db: &DatabaseConnection,
    id: i64,
    canonical_url: &str,
) -> anyhow::Result<Model, DbErr> {
    let task = Entity::find_by_id(id).one(db).await?;
    if let Some(task) = task {
        let existing_task = Entity::find()
            .filter(Column::Url.eq(canonical_url))
            .one(db)
            .await?;

        // Task already exists w/ this URL, mark this one as failed.
        if let Some(existing) = existing_task {
            if existing.id != id {
                let mut data_map = HashMap::new();
                data_map.insert("canonical_url", canonical_url);

                let mut update: ActiveModel = task.into();
                update.status = Set(CrawlStatus::Failed);
                if let Ok(data) = serde_json::to_string(&data_map) {
                    update.data = Set(Some(data));
                }

                update.error = Set(Some(TaskError {
                    error_type: TaskErrorType::Duplicate,
                    msg: "Found different canonical URL".to_string(),
                }));
                let _ = update.save(db).await?;
            }

            Ok(existing)
        } else if task.url != canonical_url {
            let mut data_map = HashMap::new();
            data_map.insert("canonical_url", canonical_url);

            // Mark old task as a failed duplicate
            let mut task_update: ActiveModel = task.clone().into();
            task_update.status = Set(CrawlStatus::Failed);
            if let Ok(data) = serde_json::to_string(&data_map) {
                task_update.data = Set(Some(data));
            }

            task_update.error = Set(Some(TaskError {
                error_type: TaskErrorType::Duplicate,
                msg: "Found different canonical URL".to_string(),
            }));
            let _ = task_update.save(db).await?;

            let tags = task
                .find_related(tag::Entity)
                .all(db)
                .await
                .unwrap_or_default()
                .iter()
                .map(|m| m.id)
                .collect::<Vec<_>>();

            let mut canonical = ActiveModel::new();
            canonical.domain = Set(task.domain);
            canonical.url = Set(canonical_url.to_string());
            canonical.status = Set(CrawlStatus::Completed);

            let inserted = canonical.insert(db).await?;
            insert_tags_by_id(db, &[inserted.clone()], &tags).await?;

            Ok(inserted)
        } else {
            Ok(task)
        }
    } else {
        // deleted?
        Err(DbErr::Custom("Task not found".to_owned()))
    }
}

/// Delete all crawl tasks associated with a lens.
pub async fn delete_by_lens(db: DatabaseConnection, name: &str) -> Result<(), sea_orm::DbErr> {
    if let Ok(ids) = find_by_lens(db.clone(), name).await {
        let dbids: Vec<i64> = ids.iter().map(|item| item.id).collect();
        delete_many_by_id(&db, &dbids).await?;
    }
    Ok(())
}

/// Helper method used to delete multiple crawl entries by id. This method will first
/// delete all related tag references before deleting the crawl entries
pub async fn delete_many_by_id(
    db: &DatabaseConnection,
    dbids: &[i64],
) -> Result<u64, sea_orm::DbErr> {
    let mut rows_affected = 0;
    for chunk in dbids.chunks(BATCH_SIZE) {
        // Delete all associated tags
        crawl_tag::Entity::delete_many()
            .filter(crawl_tag::Column::CrawlQueueId.is_in(chunk.to_owned()))
            .exec(db)
            .await?;

        // Delete item
        let res = Entity::delete_many()
            .filter(Column::Id.is_in(chunk.to_owned()))
            .exec(db)
            .await?;

        rows_affected += res.rows_affected;
    }

    Ok(rows_affected)
}

/// Helper method used to delete multiple crawl entries by url. This method will first
/// delete all related tag references before deleting the crawl entries
pub async fn delete_many_by_url(
    db: &DatabaseConnection,
    urls: &[String],
) -> Result<u64, sea_orm::DbErr> {
    let mut num_deleted = 0;
    for chunk in urls.chunks(BATCH_SIZE) {
        let entries = Entity::find()
            .filter(Column::Url.is_in(chunk.to_owned()))
            .all(db)
            .await?;

        let id_list = entries.iter().map(|entry| entry.id).collect::<Vec<i64>>();

        num_deleted += delete_many_by_id(db, &id_list).await?;
    }

    Ok(num_deleted)
}

#[derive(Debug, FromQueryResult)]
pub struct CrawlTaskId {
    pub id: i64,
}

pub async fn find_by_lens(
    db: DatabaseConnection,
    name: &str,
) -> Result<Vec<CrawlTaskId>, sea_orm::DbErr> {
    CrawlTaskId::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT
            crawl_queue.id
        FROM crawl_queue
        LEFT JOIN crawl_tag on crawl_queue.id = crawl_tag.crawl_queue_id
        LEFT JOIN tags on tags.id = crawl_tag.tag_id
        WHERE tags.label = "lens" AND tags.value = $1"#,
        vec![name.into()],
    ))
    .all(&db)
    .await
}

#[derive(Debug, FromQueryResult)]
pub struct CrawlTaskIdsUrls {
    pub id: i64,
    pub url: String,
}

/// Helper method used to get the details for the task. This method will return the associated task and any
/// associated tags
pub async fn get_task_details(
    task_id: i64,
    db: &DatabaseConnection,
) -> Result<Option<(Model, Vec<tag::Model>)>, DbErr> {
    if let Some(task) = Entity::find()
        .filter(Column::Id.eq(task_id))
        .one(db)
        .await?
    {
        let tags = task.find_related(tag::Entity).all(db).await?;
        return Ok(Some((task, tags)));
    }

    Ok(None)
}

// Helper method to copy the table from one database to another
pub async fn copy_table(
    from: &DatabaseConnection,
    to: &DatabaseConnection,
) -> anyhow::Result<(), sea_orm::DbErr> {
    let mut pages = Entity::find().paginate(from, 1000);
    Entity::delete_many().exec(to).await?;
    while let Ok(Some(pages)) = pages.fetch_and_next().await {
        let active_model = pages
            .into_iter()
            .map(|model| model.into())
            .collect::<Vec<ActiveModel>>();
        Entity::insert_many(active_model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns(vec![Column::Id])
                    .do_nothing()
                    .to_owned(),
            )
            .exec(to)
            .await?;
    }
    Ok(())
}

// Helper method used to process the url sanitization configuration for
// the provided url
fn sanitize_url(url: &mut Url, config: &UrlSanitizeConfig) {
    if config.remove_query_parameter {
        url.set_query(None);
    }
}

#[cfg(test)]
mod test {
    use sea_orm::prelude::*;
    use sea_orm::{ActiveModelTrait, Set};
    use url::Url;

    use shared::config::{LensConfig, LensRule, Limit, UserSettings};
    use shared::regex::{regex_for_robots, WildcardType};

    use crate::models::crawl_queue::{CrawlStatus, CrawlType};
    use crate::models::{crawl_queue, indexed_document};
    use crate::test::setup_test_db;

    use super::{filter_urls, gen_dequeue_sql, EnqueueSettings};

    #[tokio::test]
    async fn test_insert() {
        let db = setup_test_db().await;

        let url = "oldschool.runescape.wiki/";
        let crawl = crawl_queue::ActiveModel {
            domain: Set("oldschool.runescape.wiki".to_string()),
            url: Set(url.to_owned()),
            ..Default::default()
        };
        crawl.insert(&db).await.expect("Unable to insert");

        let query = crawl_queue::Entity::find()
            .filter(crawl_queue::Column::Url.eq(url.to_string()))
            .one(&db)
            .await
            .expect("Unable to run query");

        assert!(query.is_some());

        let res = query.unwrap();
        assert_eq!(res.url, url);
    }

    #[tokio::test]
    async fn test_priority_sql() {
        let db = setup_test_db().await;

        let settings = UserSettings::default();
        let sql = gen_dequeue_sql(&db, &settings);
        assert_eq!(
            sql.to_string(),
            "WITH\nindexed AS (\n    SELECT\n        domain,\n        count(*) as count\n    FROM indexed_document\n    GROUP BY domain\n),\ninflight AS (\n    SELECT\n        domain,\n        count(*) as count\n    FROM crawl_queue\n    WHERE status = \"Processing\"\n    GROUP BY domain\n)\nSELECT\n    cq.*\nFROM crawl_queue cq\nLEFT JOIN indexed ON indexed.domain = cq.domain\nLEFT JOIN inflight ON inflight.domain = cq.domain\nWHERE\n    COALESCE(indexed.count, 0) < 500000 AND\n    COALESCE(inflight.count, 0) < 2 AND\n    status = \"Queued\" and\n    url not like \"file%\"\nORDER BY\n    cq.updated_at ASC"
        );
    }

    #[tokio::test]
    async fn test_enqueue() {
        let settings = UserSettings::default();
        let db = setup_test_db().await;
        let url = vec!["https://oldschool.runescape.wiki/".into()];
        let lens = LensConfig {
            domains: vec!["oldschool.runescape.wiki".into()],
            ..Default::default()
        };

        crawl_queue::enqueue_all(
            &db,
            &url,
            &[lens],
            &settings,
            &Default::default(),
            Option::None,
        )
        .await
        .unwrap();

        let crawl = crawl_queue::Entity::find()
            .filter(crawl_queue::Column::Url.eq(url[0].to_string()))
            .all(&db)
            .await
            .unwrap();

        assert_eq!(crawl.len(), 1);
    }

    #[tokio::test]
    async fn test_enqueue_with_recrawl() {
        let settings = UserSettings::default();
        let db = setup_test_db().await;
        let url = "https://oldschool.runescape.wiki/".to_owned();

        let _ = crawl_queue::Entity::insert(crawl_queue::ActiveModel {
            domain: Set("oldschool.runescape.wiki".into()),
            crawl_type: Set(crawl_queue::CrawlType::Bootstrap),
            url: Set(url.clone()),
            status: Set(crawl_queue::CrawlStatus::Completed),
            ..Default::default()
        })
        .exec(&db)
        .await;

        let overrides = crawl_queue::EnqueueSettings {
            force_allow: true,
            is_recrawl: true,
            ..Default::default()
        };

        let all = crawl_queue::Entity::find()
            .filter(crawl_queue::Column::Status.eq(crawl_queue::CrawlStatus::Completed))
            .all(&db)
            .await
            .unwrap();

        assert_eq!(all.len(), 1);

        crawl_queue::enqueue_all(&db, &[url], &[], &settings, &overrides, Option::None)
            .await
            .unwrap();

        let res = crawl_queue::Entity::find()
            .filter(crawl_queue::Column::Status.eq(crawl_queue::CrawlStatus::Queued))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(res.len(), 1);
    }

    #[tokio::test]
    async fn test_enqueue_with_rules() {
        let settings = UserSettings::default();
        let db = setup_test_db().await;
        let url = vec!["https://oldschool.runescape.wiki/w/Worn_Equipment?veaction=edit".into()];
        let lens = LensConfig {
            domains: vec!["oldschool.runescape.wiki".into()],
            rules: vec![LensRule::SkipURL(
                "https://oldschool.runescape.wiki/*veaction=*".into(),
            )],
            ..Default::default()
        };

        crawl_queue::enqueue_all(
            &db,
            &url,
            &[lens],
            &settings,
            &Default::default(),
            Option::None,
        )
        .await
        .unwrap();

        let crawl = crawl_queue::Entity::find()
            .filter(crawl_queue::Column::Url.eq(url[0].to_string()))
            .all(&db)
            .await
            .unwrap();

        assert_eq!(crawl.len(), 0);
    }

    #[tokio::test]
    async fn test_dequeue() {
        let settings = UserSettings::default();
        let db = setup_test_db().await;
        let url = vec!["https://oldschool.runescape.wiki/".into()];
        let lens = LensConfig {
            domains: vec!["oldschool.runescape.wiki".into()],
            ..Default::default()
        };

        crawl_queue::enqueue_all(
            &db,
            &url,
            &[lens],
            &settings,
            &Default::default(),
            Option::None,
        )
        .await
        .unwrap();

        let queue = crawl_queue::dequeue(&db, &settings).await.unwrap();

        assert!(queue.is_some());
        assert_eq!(queue.unwrap().url, url[0]);
    }

    #[tokio::test]
    async fn test_dequeue_with_limit() {
        let settings = UserSettings {
            domain_crawl_limit: Limit::Finite(2),
            ..Default::default()
        };
        let db = setup_test_db().await;
        let url: Vec<String> = vec!["https://oldschool.runescape.wiki/".into()];
        let parsed = Url::parse(&url[0]).unwrap();
        let lens = LensConfig {
            domains: vec!["oldschool.runescape.wiki".into()],
            ..Default::default()
        };

        crawl_queue::enqueue_all(
            &db,
            &url,
            &[lens],
            &settings,
            &Default::default(),
            Option::None,
        )
        .await
        .unwrap();
        let doc = indexed_document::ActiveModel {
            domain: Set(parsed.host_str().unwrap().to_string()),
            url: Set(url[0].clone()),
            doc_id: Set("docid".to_string()),
            ..Default::default()
        };
        doc.save(&db).await.unwrap();
        let queue = crawl_queue::dequeue(&db, &settings).await.unwrap();
        assert!(queue.is_some());

        let settings = UserSettings {
            domain_crawl_limit: Limit::Finite(1),
            ..Default::default()
        };
        let queue = crawl_queue::dequeue(&db, &settings).await.unwrap();
        assert!(queue.is_none());
    }

    #[tokio::test]
    async fn test_remove_by_rule() {
        let settings = UserSettings::default();
        let db = setup_test_db().await;
        let overrides = EnqueueSettings::default();

        let lens = LensConfig {
            domains: vec!["en.wikipedia.com".into()],
            ..Default::default()
        };

        let urls: Vec<String> = vec![
            "https://en.wikipedia.com/".into(),
            "https://en.wikipedia.org/wiki/Rust_(programming_language)".into(),
            "https://en.wikipedia.com/wiki/Mozilla".into(),
            "https://en.wikipedia.com/wiki/Cheese?id=13314&action=edit".into(),
            "https://en.wikipedia.com/wiki/Testing?action=edit".into(),
        ];

        crawl_queue::enqueue_all(&db, &urls, &[lens], &settings, &overrides, Option::None)
            .await
            .unwrap();

        let rule = "https://en.wikipedia.com/*action=*";
        let regex = regex_for_robots(rule, WildcardType::Database).unwrap();
        let removed = super::remove_by_rule(&db, &regex).await.unwrap();
        assert_eq!(removed, 2);
    }

    #[tokio::test]
    async fn test_create_ruleset() {
        let lens =
            LensConfig::from_string(include_str!("../../../../fixtures/lens/test.ron")).unwrap();

        let rules = super::create_ruleset_from_lens(&lens);
        let allow_list = regex::RegexSet::new(rules.allow_list).unwrap();
        let block_list = regex::RegexSet::new(rules.skip_list).unwrap();

        let valid = "https://walkingdead.fandom.com/wiki/18_Miles_Out";
        let invalid = "https://walkingdead.fandom.com/wiki/Aaron_(Comic_Series)/Gallery";

        assert!(allow_list.is_match(valid));
        assert!(!block_list.is_match(valid));
        // Allowed without the SkipURL
        assert!(allow_list.is_match(invalid));
        // but should now be denied
        assert!(block_list.is_match(invalid));
    }

    #[tokio::test]
    async fn test_create_ruleset_with_limits() {
        let lens =
            LensConfig::from_string(include_str!("../../../../fixtures/lens/imdb.ron")).unwrap();

        let rules = super::create_ruleset_from_lens(&lens);
        let allow_list = regex::RegexSet::new(rules.allow_list).unwrap();
        let block_list = regex::RegexSet::new(rules.skip_list).unwrap();
        let restrict_list = regex::RegexSet::new(rules.restrict_list).unwrap();

        let valid = vec![
            "https://www.imdb.com/title/tt0094625",
            "https://www.imdb.com/title/tt0094625/",
            "https://www.imdb.com/title",
            "https://www.imdb.com/title/",
        ];

        let invalid = vec![
            // Bare domain should not match
            "https://www.imdb.com",
            // Matches the URL depth but does not match the URL prefix.
            "https://www.imdb.com/blah/blah",
            // Pages past the detail page should not match.
            "https://www.imdb.com/title/tt0094625/reviews",
            // Should block URLs that are skipped but match restrictions
            "https://www.imdb.com/title/fake_title",
        ];

        for url in valid {
            assert!(allow_list.is_match(url));
            // All valid URLs should match the restriction as well.
            assert!(restrict_list.is_match(url));
            assert!(!block_list.is_match(url));
        }

        for url in invalid {
            // Allowed, but then restricted by rules.
            if allow_list.is_match(url) {
                assert!(!restrict_list.is_match(url) || block_list.is_match(url));
            } else {
                // Other not allowed at all
                assert!(!allow_list.is_match(url));
            }
        }
    }

    #[test]
    fn test_filter_urls() {
        let settings = UserSettings::default();
        let overrides = EnqueueSettings::default();

        let lens =
            LensConfig::from_string(include_str!("../../../../fixtures/lens/bahai.ron")).unwrap();

        let to_enqueue = vec![
            "https://bahai-library.com//shoghi-effendi_goals_crusade".into(),
            "https://www.stumbleupon.com/submit?url=https://bahaiworld.bahai.org/library/western-liberal-democracy-as-new-world-order/&title=Western%20Liberal%20Democracy%20as%20New%20World%20Order?".into(),
            "https://www.reddit.com/submit?title=The%20Epic%20of%20Humanity&url=https://bahaiworld.bahai.org/library/the-epic-of-humanity".into()
        ];

        let mut filtered = filter_urls(&[lens], &settings, &overrides, &to_enqueue)
            .expect("Unable to filter urls");
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered.pop(),
            Some("https://bahai-library.com//shoghi-effendi_goals_crusade".into())
        );
    }

    #[tokio::test]
    async fn test_update_or_remove_task() {
        let db = setup_test_db().await;

        let model = crawl_queue::ActiveModel {
            crawl_type: Set(CrawlType::Normal),
            domain: Set("example.com".to_string()),
            status: Set(crawl_queue::CrawlStatus::Completed),
            url: Set("https://example.com".to_string()),
            ..Default::default()
        };
        let first = model.insert(&db).await.expect("saved");

        let model = crawl_queue::ActiveModel {
            crawl_type: Set(CrawlType::Normal),
            domain: Set("example.com".to_string()),
            status: Set(crawl_queue::CrawlStatus::Completed),
            url: Set("https://example.com/redirect".to_string()),
            ..Default::default()
        };
        let task = model.insert(&db).await.expect("saved");

        let res = super::update_or_remove_task(&db, task.id, "https://example.com")
            .await
            .expect("success");

        let all_tasks = crawl_queue::Entity::find().all(&db).await.expect("success");

        let task = crawl_queue::Entity::find_by_id(task.id)
            .one(&db)
            .await
            .expect("success")
            .expect("should exist");
        // Old task should be marked as failed
        assert_eq!(task.status, CrawlStatus::Failed);
        // New model should have the canonical URL.
        assert_eq!(res.url, "https://example.com");
        assert_eq!(res.id, first.id);
        assert_eq!(2, all_tasks.len());
    }
}
