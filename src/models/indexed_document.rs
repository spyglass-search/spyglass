use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "indexed_document")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub url: String,

    /// Reference to the document in the index
    pub doc_addr_segment: u32,
    pub doc_addr_id: u32,
    /// Location on disk
    pub path: String,
    pub indexed_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No RelationDef")
    }
}

impl ActiveModelBehavior for ActiveModel {}

// impl IndexedDocument {
//     pub async fn init_table(db: &DbPool) -> anyhow::Result<(), sqlx::Error> {
//         let mut conn = db.acquire().await?;

//         sqlx::query(
//             "CREATE TABLE IF NOT EXISTS indexed_document (
//                 id INTEGER PRIMARY KEY,
//                 url TEXT UNIQUE,
//                 doc_addr_segment INTEGER,
//                 doc_addr_id INTEGER,
//                 indexed_at DATETIME default CURRENT_TIMESTAMP
//             )",
//         )
//         .execute(&mut conn)
//         .await?;

//         Ok(())
//     }
// }
