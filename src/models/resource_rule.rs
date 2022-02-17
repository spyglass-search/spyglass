use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "resource_rules")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub domain: String,
    pub rule: String,
    pub no_index: bool,
    pub allow_crawl: bool,
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

impl ActiveModelBehavior for ActiveModel {}

// impl ResourceRule {
//     pub async fn find(db: &DbPool, domain: &str) -> anyhow::Result<Vec<ResourceRule>, sqlx::Error> {
//         let mut conn = db.acquire().await?;

//         let rows = sqlx::query(
//             "SELECT
//                 id,
//                 domain,
//                 rule,
//                 no_index,
//                 allow_crawl,
//                 created_at,
//                 updated_at
//             FROM resource_rules
//             WHERE domain = ?",
//         )
//         .bind(domain)
//         .try_map(|row: SqliteRow| {
//             let rule_str = row.get(2);
//             let rule = ResourceRule {
//                 id: row.get::<Option<i64>, _>(0),
//                 domain: row.get(1),
//                 rule: Regex::new(rule_str).unwrap(),
//                 no_index: row.get(3),
//                 allow_crawl: row.get(4),
//                 created_at: row.get(5),
//                 updated_at: row.get(6),
//             };

//             Ok(rule)
//         })
//         .fetch_all(&mut conn)
//         .await?;

//         Ok(rows)
//     }

//     pub async fn insert(
//         db: &DbPool,
//         domain: &str,
//         rule: &str,
//         no_index: bool,
//         allow_crawl: bool,
//     ) -> Result<(), sqlx::Error> {
//         let mut conn = db.acquire().await?;

//         sqlx::query(
//             "INSERT INTO resource_rules (domain, rule, no_index, allow_crawl)
//             VALUES (?, ?, ?, ?)",
//         )
//         .bind(domain)
//         .bind(rule)
//         .bind(no_index)
//         .bind(allow_crawl)
//         .execute(&mut conn)
//         .await?;

//         Ok(())
//     }

//     pub async fn insert_rule(db: &DbPool, rule: &ResourceRule) -> Result<(), sqlx::Error> {
//         let mut conn = db.acquire().await?;

//         sqlx::query(
//             "INSERT INTO resource_rules (domain, rule, no_index, allow_crawl)
//             VALUES (?1, ?2, ?3, ?4)",
//         )
//         .bind(&rule.domain)
//         .bind(&rule.rule.to_string())
//         .bind(rule.no_index)
//         .bind(rule.allow_crawl)
//         .execute(&mut conn)
//         .await?;

//         Ok(())
//     }
// }

// #[cfg(test)]
// mod test {
//     use crate::config::Config;
//     use crate::models::{create_connection, ResourceRule};
//     use std::path::Path;

//     #[tokio::test]
//     async fn test_insert() -> anyhow::Result<(), sqlx::Error> {
//         let config = Config {
//             data_dir: Path::new("/tmp").to_path_buf(),
//             prefs_dir: Path::new("/tmp").to_path_buf(),
//         };

//         let db = create_connection(&config).await.unwrap();
//         ResourceRule::init_table(&db).await?;

//         let res = ResourceRule::insert(&db, "oldschool.runescape.wiki", "/", false, true).await;
//         assert!(res.is_ok());

//         let rules = ResourceRule::find(&db, "oldschool.runescape.wiki")
//             .await
//             .expect("Unable to find rules");
//         assert_eq!(rules.len(), 1);

//         Ok(())
//     }
// }
