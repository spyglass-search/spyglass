use std::collections::HashSet;

use regex::RegexSet;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{OnConflict, SqliteQueryBuilder};
use sea_orm::{sea_query, ConnectionTrait, DbBackend, FromQueryResult, QueryTrait, Set, Statement};
use serde::{Deserialize, Serialize};
use url::Url;

use super::indexed_document;
use shared::config::{LensConfig, LensRule, Limit, UserSettings};
use shared::regex::{regex_for_domain, regex_for_prefix};

const MAX_RETRIES: u8 = 5;
const BATCH_SIZE: usize = 5_000;

#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, Eq)]
#[sea_orm(rs_type = "String", db_type = "String(None)")]
pub enum TaskErrorType {
    #[sea_orm(string_value = "Collect")]
    Collect,
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
    #[sea_orm(string_value = "Queued")]
    Queued,
    #[sea_orm(string_value = "Processing")]
    Processing,
    #[sea_orm(string_value = "Completed")]
    Completed,
    #[sea_orm(string_value = "Failed")]
    Failed,
}

#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Eq)]
#[sea_orm(rs_type = "String", db_type = "String(None)")]
pub enum CrawlType {
    #[sea_orm(string_value = "API")]
    Api,
    #[sea_orm(string_value = "Bootstrap")]
    Bootstrap,
    #[sea_orm(string_value = "Normal")]
    Normal,
}

impl Default for CrawlType {
    fn default() -> Self {
        CrawlType::Normal
    }
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
            crawl_type: Set(CrawlType::Normal),
            status: Set(CrawlStatus::Queued),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }

    // Triggered before insert / update
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        if !insert {
            self.updated_at = Set(chrono::Utc::now());
        }

        Ok(self)
    }
}

pub async fn queue_stats(
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<QueueCountByStatus>, sea_orm::DbErr> {
    let res = Entity::find()
        .from_raw_sql(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT count(*) as count, domain, status FROM crawl_queue GROUP BY domain, status"
                .into(),
        ))
        .into_model::<QueueCountByStatus>()
        .all(db)
        .await?;

    Ok(res)
}

pub async fn reset_processing(db: &DatabaseConnection) -> anyhow::Result<()> {
    Entity::update_many()
        .col_expr(Column::Status, sea_query::Expr::value(CrawlStatus::Queued))
        .filter(Column::Status.eq(CrawlStatus::Processing))
        .exec(db)
        .await?;

    Ok(())
}

#[derive(Debug, FromQueryResult)]
pub struct QueueCountByStatus {
    pub count: i64,
    pub domain: String,
    pub status: String,
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

fn gen_priority_values(items: &[String], is_prefix: bool) -> String {
    if items.is_empty() {
        "(\"\", 0)".to_string()
    } else {
        items
            .iter()
            .map(|item| {
                let item = if is_prefix {
                    // Wildcards not supported in prefixes
                    item.to_owned() + "%"
                } else {
                    // TODO: Should probably sanitize this...
                    item.replace('*', "%")
                };

                format!("(\"{}\", 1)", item)
            })
            .collect::<Vec<String>>()
            .join(",")
    }
}

fn gen_priority_sql(p_domains: &str, p_prefixes: &str, user_settings: UserSettings) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Sqlite,
        &format!(
            r#"WITH
                p_domain(domain, priority) AS (values {}),
                p_prefix(prefix, priority) AS (values {}), {}"#,
            p_domains,
            p_prefixes,
            include_str!("sql/dequeue.sqlx")
        ),
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
        }
    }

    LensRuleSets {
        allow_list,
        skip_list,
        restrict_list,
    }
}

/// Get the next url in the crawl queue
pub async fn dequeue(
    db: &DatabaseConnection,
    user_settings: UserSettings,
    // Prioritized domains
    p_domains: &[String],
    // Prioritized prefixes
    p_prefixes: &[String],
) -> anyhow::Result<Option<Model>, sea_orm::DbErr> {
    // Check for inflight limits
    if let Limit::Finite(inflight_crawl_limit) = user_settings.inflight_crawl_limit {
        // How many do we have in progress?
        let num_in_progress = Entity::find()
            .filter(Column::Status.eq(CrawlStatus::Processing))
            .count(db)
            .await? as u32;

        if num_in_progress >= inflight_crawl_limit {
            return Ok(None);
        }
    }

    // Prioritize any bootstrapping tasks first.
    let entity = Entity::find()
        .filter(Column::Status.eq(CrawlStatus::Queued))
        .filter(Column::CrawlType.eq(CrawlType::Bootstrap))
        .one(db)
        .await?;

    if let Some(task) = entity {
        let mut update: ActiveModel = task.into();
        update.status = Set(CrawlStatus::Processing);
        let model: Model = update.update(db).await.expect("Unable to save");
        return Ok(Some(model));
    }

    // List of domains to prioritize when dequeuing tasks
    // For example, we'll pull domains that make up with lenses before
    // general crawling.
    let prioritized_domains = gen_priority_values(p_domains, false);
    let prioritized_prefixes = gen_priority_values(p_prefixes, true);

    let entity = Entity::find().from_raw_sql(gen_priority_sql(
        &prioritized_domains,
        &prioritized_prefixes,
        user_settings,
    ));

    // Grab new entity and immediately mark in-progress
    if let Ok(Some(model)) = entity.one(db).await {
        let mut update: ActiveModel = model.into();
        update.status = Set(CrawlStatus::Processing);
        let model: Model = update.update(db).await.expect("Unable to save");
        return Ok(Some(model));
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
    pub force_allow: bool,
    pub is_recrawl: bool,
}

fn filter_urls(
    lenses: &[LensConfig],
    settings: &UserSettings,
    overrides: &EnqueueSettings,
    urls: &[String],
) -> Vec<String> {
    let mut allow_list: Vec<String> = Vec::new();
    let mut skip_list: Vec<String> = Vec::new();
    let mut restrict_list: Vec<String> = Vec::new();

    for domain in settings.block_list.iter() {
        skip_list.push(regex_for_domain(domain));
    }

    for lens in lenses {
        let ruleset = create_ruleset_from_lens(lens);
        allow_list.extend(ruleset.allow_list);
        skip_list.extend(ruleset.skip_list);
        restrict_list.extend(ruleset.restrict_list);
    }

    let allow_list = RegexSet::new(allow_list).expect("Unable to create allow list");
    let skip_list = RegexSet::new(skip_list).expect("Unable to create skip list");
    let restrict_list = RegexSet::new(restrict_list).expect("Unable to create restrict list");

    // Ignore invalid URLs
    urls.iter()
        .filter_map(|url| {
            if let Ok(mut parsed) = Url::parse(url) {
                // Check that we can handle this scheme
                if parsed.scheme() != "http"
                    && parsed.scheme() != "https"
                    && parsed.scheme() != "file"
                    && parsed.scheme() != "api"
                {
                    return None;
                }

                // Always ignore fragments, otherwise crawling
                // https://wikipedia.org/Rust#Blah would be considered different than
                // https://wikipedia.org/Rust
                parsed.set_fragment(None);

                let normalized = parsed.to_string();

                // Ignore domains on blacklist
                if skip_list.is_match(&normalized)
                    // Skip if any URLs do not match this restriction
                    || (!restrict_list.is_empty()
                        && !restrict_list.is_match(&normalized))
                {
                    return None;
                }

                // Should we crawl external links?
                if settings.crawl_external_links {
                    return Some(normalized);
                }

                // If external links are not allowed, only allow crawls specified
                // in our lenses
                if overrides.force_allow
                    || (!allow_list.is_empty() && allow_list.is_match(&normalized))
                {
                    return Some(normalized);
                }
            }

            None
        })
        .collect::<Vec<String>>()
}

pub async fn enqueue_all(
    db: &DatabaseConnection,
    urls: &[String],
    lenses: &[LensConfig],
    settings: &UserSettings,
    overrides: &EnqueueSettings,
    pipeline: Option<String>,
) -> anyhow::Result<(), sea_orm::DbErr> {
    // Filter URLs
    let urls = filter_urls(lenses, settings, overrides, urls);

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
                        _ => parsed.host_str().expect("Invalid URL host"),
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

        let (sql, values) = Entity::insert_many(owned)
            .query()
            .on_conflict(on_conflict.clone())
            .build(SqliteQueryBuilder);

        let values: Vec<Value> = values.iter().map(|x| x.to_owned()).collect();
        match db
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                &sql,
                values,
            ))
            .await
        {
            Ok(_) => {}
            Err(e) => log::error!("insert_many error: {:?}", e),
        }
    }

    Ok(())
}

pub async fn mark_done(
    db: &DatabaseConnection,
    id: i64,
    status: CrawlStatus,
) -> anyhow::Result<()> {
    if let Some(crawl) = Entity::find_by_id(id).one(db).await? {
        let mut updated: ActiveModel = crawl.clone().into();

        // Bump up number of retries if this failed
        if status == CrawlStatus::Failed && crawl.num_retries <= MAX_RETRIES {
            updated.num_retries = Set(crawl.num_retries + 1);
            // Queue again
            updated.status = Set(CrawlStatus::Queued);
        } else {
            updated.status = Set(status);
        }

        updated.update(db).await?;
    }

    Ok(())
}

/// Remove tasks from the crawl queue that match `rule`. Rule is expected
/// to be a SQL like statement.
pub async fn remove_by_rule(db: &DatabaseConnection, rule: &str) -> anyhow::Result<u64> {
    let res = Entity::delete_many()
        .filter(Column::Url.like(rule))
        .exec(db)
        .await?;

    if res.rows_affected > 0 {
        log::info!("removed {} tasks due to '{}'", res.rows_affected, rule);
    }
    Ok(res.rows_affected)
}

#[cfg(test)]
mod test {
    use sea_orm::prelude::*;
    use sea_orm::{ActiveModelTrait, Set};
    use url::Url;

    use shared::config::{LensConfig, LensRule, Limit, UserSettings};
    use shared::regex::{regex_for_robots, WildcardType};

    use crate::models::{crawl_queue, indexed_document};
    use crate::test::setup_test_db;

    use super::{filter_urls, gen_priority_sql, gen_priority_values, EnqueueSettings};

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

    #[test]
    fn test_priority_sql() {
        let settings = UserSettings::default();
        let p_domains = gen_priority_values(&["en.wikipedia.org".to_string()], false);
        let p_prefixes =
            gen_priority_values(&["https://roll20.net/compendium/dnd5e".to_string()], true);

        let sql = gen_priority_sql(&p_domains, &p_prefixes, settings);
        assert_eq!(
            sql.to_string(),
            "WITH\n                p_domain(domain, priority) AS (values (\"en.wikipedia.org\", 1)),\n                p_prefix(prefix, priority) AS (values (\"https://roll20.net/compendium/dnd5e%\", 1)), indexed AS (\n    SELECT\n        domain,\n        count(*) as count\n    FROM indexed_document\n    GROUP BY domain\n),\ninflight AS (\n    SELECT\n        domain,\n        count(*) as count\n    FROM crawl_queue\n    WHERE status = \"Processing\"\n    GROUP BY domain\n)\nSELECT\n    cq.*\nFROM crawl_queue cq\nLEFT JOIN p_domain ON cq.domain like p_domain.domain\nLEFT JOIN p_prefix ON cq.url like p_prefix.prefix\nLEFT JOIN indexed ON indexed.domain = cq.domain\nLEFT JOIN inflight ON inflight.domain = cq.domain\nWHERE\n    COALESCE(indexed.count, 0) < 500000 AND\n    COALESCE(inflight.count, 0) < 2 AND\n    status = \"Queued\"\nORDER BY\n    p_prefix.priority DESC,\n    p_domain.priority DESC,\n    cq.updated_at ASC"
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
        let prioritized = vec![];
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

        let queue = crawl_queue::dequeue(&db, settings, &prioritized, &[])
            .await
            .unwrap();

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
        let prioritized = vec![];
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
        let queue = crawl_queue::dequeue(&db, settings, &prioritized, &[])
            .await
            .unwrap();
        assert!(queue.is_some());

        let settings = UserSettings {
            domain_crawl_limit: Limit::Finite(1),
            ..Default::default()
        };
        let queue = crawl_queue::dequeue(&db, settings, &prioritized, &[])
            .await
            .unwrap();
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

        let mut filtered = filter_urls(&[lens], &settings, &overrides, &to_enqueue);
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered.pop(),
            Some("https://bahai-library.com//shoghi-effendi_goals_crusade".into())
        );
    }
}
