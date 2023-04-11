use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Eq)]
#[sea_orm(rs_type = "String", db_type = "String(Some(1))")]
pub enum FetchProtocol {
    #[sea_orm(string_value = "HTTP")]
    Http,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "fetch_history")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Protocol
    pub protocol: FetchProtocol,
    /// Domain
    pub domain: String,
    /// Path fetched at this URL
    pub path: String,
    /// Hash used to check for changes.
    pub hash: Option<String>,
    /// HTTP status when last fetching this page.
    pub status: u16,
    /// Ignore this URL in the future.
    #[sea_orm(default_value = false)]
    pub no_index: bool,
    /// When this was first added to our fetch history
    pub created_at: DateTimeUtc,
    /// When this URL was last fetched.
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
    fn new() -> Self {
        Self {
            protocol: Set(FetchProtocol::Http),
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

pub async fn find_by_url(
    db: &DatabaseConnection,
    url: &Url,
) -> anyhow::Result<Option<Model>, sea_orm::DbErr> {
    Entity::find()
        .filter(Column::Domain.eq(url.host_str().unwrap_or_default().to_string()))
        .filter(Column::Path.eq(url.path()))
        .one(db)
        .await
}

pub async fn upsert(
    db: &DatabaseConnection,
    domain: &str,
    path: &str,
    hash: Option<String>,
    status: u16,
) -> anyhow::Result<Model, sea_orm::DbErr> {
    let history = Entity::find()
        .filter(Column::Domain.eq(domain))
        .filter(Column::Path.eq(path))
        .one(db)
        .await?;

    match history {
        // Already exists, update
        Some(res) => {
            let mut model: ActiveModel = res.into();
            model.hash = Set(hash.to_owned());
            model.status = Set(status);
            model.updated_at = Set(chrono::Utc::now());
            Ok(model.update(db).await?)
        }
        // Doesn't exist, insert into db
        None => {
            let new_hist = ActiveModel {
                domain: Set(domain.to_owned()),
                path: Set(path.to_owned()),
                hash: Set(hash.to_owned()),
                status: Set(status),
                ..Default::default()
            };

            Ok(new_hist.insert(db).await?)
        }
    }
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

#[cfg(test)]
mod test {
    use sea_orm::prelude::*;
    use sea_orm::{ActiveModelTrait, Set};

    use crate::models::fetch_history;
    use crate::test::setup_test_db;

    #[tokio::test]
    async fn test_insert() {
        let db = setup_test_db().await;

        let hash = "this is a hash".to_string();
        let new = fetch_history::ActiveModel {
            domain: Set("oldschool.runescape.wiki".to_owned()),
            path: Set("/".to_owned()),
            hash: Set(Some(hash.to_owned())),
            status: Set(200),
            ..Default::default()
        };

        new.insert(&db).await.unwrap();

        let domain = "oldschool.runescape.wiki";
        let path = "/";

        let history = fetch_history::Entity::find()
            .filter(fetch_history::Column::Domain.eq(domain))
            .filter(fetch_history::Column::Path.eq(path))
            .one(&db)
            .await
            .unwrap();

        assert!(history.is_some());

        let res = history.unwrap();
        assert_eq!(res.domain, domain);
        assert_eq!(res.path, path);
        assert_eq!(res.hash.unwrap(), hash);
    }
}
