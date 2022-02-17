use sea_orm::entity::prelude::*;
use sea_orm::Set;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "fetch_history")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// URL fetched.
    pub url: String,
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

impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }
}

pub async fn upsert(
    db: &DatabaseConnection,
    url_base: &str,
    hash: Option<String>,
    status: u16,
) -> anyhow::Result<Model, sea_orm::DbErr> {
    let history = Entity::find()
        .filter(Column::Url.eq(url_base))
        .one(db)
        .await?;

    return match history {
        // Already exists, update
        Some(res) => {
            let mut model: ActiveModel = res.into();
            model.hash = Set(hash.to_owned());
            model.status = Set(status);

            Ok(model.update(db).await?)
        }
        // Doesn't exist, insert into db
        None => {
            let new_hist = ActiveModel {
                url: Set(url_base.to_owned()),
                hash: Set(hash.to_owned()),
                status: Set(status),
                ..Default::default()
            };

            Ok(new_hist.insert(db).await?)
        }
    };
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use sea_orm::prelude::*;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, Schema, Set};

    use crate::config::Config;
    use crate::models::{create_connection, fetch_history};

    #[tokio::test]
    async fn test_insert() {
        let config = Config {
            data_dir: Path::new("/tmp").to_path_buf(),
            prefs_dir: Path::new("/tmp").to_path_buf(),
        };

        // Create table
        let db = create_connection(&config, true).await.unwrap();
        let builder = db.get_database_backend();
        let schema = Schema::new(builder);
        let create = builder.build(&schema.create_table_from_entity(fetch_history::Entity));
        db.execute(create).await.unwrap();

        let hash = "this is a hash".to_string();
        let new = fetch_history::ActiveModel {
            url: Set("oldschool.runescape.wiki/".to_owned()),
            hash: Set(Some(hash.to_owned())),
            status: Set(200),
            ..Default::default()
        };
        println!("{:?}", new);
        new.insert(&db).await.unwrap();

        let url = "oldschool.runescape.wiki/";
        let history = fetch_history::Entity::find()
            .filter(fetch_history::Column::Url.eq(url.to_string()))
            .one(&db)
            .await
            .unwrap();

        assert!(history.is_some());

        let res = history.unwrap();
        assert_eq!(res.url, url);
        assert_eq!(res.hash.unwrap(), hash);
    }
}
