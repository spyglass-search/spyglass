use std::path::PathBuf;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use entities::models::schema::v3::SchemaReader;
use entities::schema::DocFields;
use sea_orm_migration::prelude::*;
use tantivy::DateTime;
use tantivy::{schema::*, IndexWriter};

use entities::schema::{self, mapping_to_schema, SchemaMapping, SearchDocument};
use entities::sea_orm::{ConnectionTrait, Statement};
use shared::config::Config;

use crate::utils::migration_utils;
pub struct Migration;
impl Migration {
    pub fn after_schema(&self) -> SchemaMapping {
        DocFields::as_field_vec()
    }

    pub fn after_writer(&self, path: &PathBuf) -> IndexWriter {
        let index = schema::initialize_index(path).expect("Unable to open search index");
        index.writer(50_000_000).expect("Unable to create writer")
    }

    pub fn migrate_document(
        &self,
        doc_id: &str,
        old_schema: &SchemaReader,
        new_schema: &Schema,
    ) -> Document {
        let mut new_doc = Document::default();
        new_doc.add_text(new_schema.get_field("id").unwrap(), doc_id);

        let txt_vals = old_schema.get_txt_values(doc_id);
        let unsigned_vals = old_schema.get_unsigned_fields(doc_id);

        for (old_field, new_field) in &[
            // Will map <old> -> <new>
            ("domain", "domain"),
            ("title", "title"),
            ("description", "description"),
            // No content was saved previous, so we'll use the description as a stopgap
            // and recrawl stuff
            ("content", "content"),
            ("url", "url"),
            ("tags", "tags"),
        ] {
            let new_field = new_schema.get_field(new_field).unwrap();

            if let Some(old_value) = txt_vals.get(old_field.to_owned()) {
                new_doc.add_text(new_field, old_value);
            }

            if let Some(old_value) = unsigned_vals.get(old_field.to_owned()) {
                for val in old_value {
                    new_doc.add_u64(new_field, *val);
                }
            }
        }

        let start = SystemTime::now();
        let unix_time = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");

        let date_time = DateTime::from_timestamp_millis(unix_time.as_millis() as i64);

        new_doc.add_date(new_schema.get_field("published").unwrap(), date_time);
        new_doc.add_date(new_schema.get_field("lastmodified").unwrap(), date_time);

        new_doc
    }
}

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20230315_000001_migrate_search_schema"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let result = manager
            .get_connection()
            .query_all(Statement::from_string(
                manager.get_database_backend(),
                "SELECT id, doc_id, url FROM indexed_document".to_owned(),
            ))
            .await?;

        let config = Config::new();
        let old_index_path = config.index_dir();
        // No docs yet, nothing to migrate.
        if result.is_empty() {
            // Removing the old index folder will also remove any metadata that lingers
            // from an empty index.
            let _ = std::fs::remove_dir_all(old_index_path);
            return Ok(());
        }

        let new_index_path = old_index_path
            .parent()
            .expect("Expected parent path")
            .join("migrated_index");

        if !new_index_path.exists() {
            if let Err(e) = std::fs::create_dir(new_index_path.clone()) {
                return Err(DbErr::Custom(format!("Can't create new index: {e}")));
            }
        }

        println!("Migrating index @ {old_index_path:?} to {new_index_path:?}");

        let old_schema = SchemaReader::new(&old_index_path);
        let new_schema = mapping_to_schema(&self.after_schema());
        if !old_schema.has_reader() {
            // Potentially already migrated?
            println!("Error opening index");
            return Ok(());
        }

        let mut new_writer = self.after_writer(&new_index_path);

        let now = Instant::now();

        let _errs = result
            .iter()
            .filter_map(|row| {
                let doc_id: String = row.try_get::<String>("", "doc_id").unwrap();

                if let Err(e) = new_writer.add_document(self.migrate_document(
                    &doc_id,
                    &old_schema,
                    &new_schema,
                )) {
                    log::error!("Error migrating doc {:?}", e);
                    return Some(DbErr::Custom(format!("Unable to migrate doc: {e}")));
                }

                None
            })
            .collect::<Vec<DbErr>>();

        // Save change to new index
        if let Err(e) = new_writer.commit() {
            return Err(DbErr::Custom(format!("Unable to commit changes: {e}")));
        }

        if let Err(e) = migration_utils::backup_dir(&old_index_path) {
            return Err(DbErr::Custom(format!("Unable to backup old index: {e}")));
        }

        // Move new index into place.
        if let Err(e) = migration_utils::replace_dir(&new_index_path, &old_index_path) {
            return Err(DbErr::Custom(format!(
                "Unable to move new index into place: {e}"
            )));
        }

        let elapsed_time = now.elapsed();
        println!("Migration took {} seconds.", elapsed_time.as_secs());

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
