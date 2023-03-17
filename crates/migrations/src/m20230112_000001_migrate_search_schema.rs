use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use entities::models::schema::v2::{self, SearchDocument as SearchDocumentV2};
use entities::models::schema::v3::{self, SearchDocument as sSearchDocumentV3};
use entities::sea_orm::QueryResult;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use sea_orm_migration::prelude::*;
use tantivy_18::collector::TopDocs;
use tantivy_18::directory::MmapDirectory;
use tantivy_18::query::TermQuery;
use tantivy_18::TantivyError;
use tantivy_18::{schema::*, IndexWriter};
use tantivy_18::{Index, IndexReader, ReloadPolicy};

use entities::sea_orm::{ConnectionTrait, Statement};
use shared::config::Config;

use crate::utils::migration_utils;

pub struct Migration;
impl Migration {
    pub fn before_schema(&self) -> v2::SchemaMapping {
        v2::DocFields::as_field_vec()
    }

    pub fn before_reader(&self, path: &PathBuf) -> Result<IndexReader, TantivyError> {
        let dir = MmapDirectory::open(path).expect("Unable to mmap search index");
        let index = Index::open_or_create(dir, v2::mapping_to_schema(&self.before_schema()))?;

        index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
    }

    pub fn after_schema(&self) -> v3::SchemaMapping {
        v3::DocFields::as_field_vec()
    }

    pub fn after_writer(&self, path: &PathBuf) -> IndexWriter {
        let dir = MmapDirectory::open(path).expect("Unable to mmap search index");
        let index = Index::open_or_create(dir, v3::mapping_to_schema(&self.after_schema()))
            .expect("Unable to open search index");

        index.writer(50_000_000).expect("Unable to create writer")
    }

    pub fn migrate_document(
        &self,
        doc_id: &str,
        old_doc: Document,
        old_schema: &Schema,
        new_schema: &Schema,
        tags: Option<&Vec<u64>>,
    ) -> Document {
        let mut new_doc = Document::default();
        new_doc.add_text(new_schema.get_field("id").unwrap(), doc_id);
        for (old_field, new_field) in &[
            // Will map <old> -> <new>
            ("domain", "domain"),
            ("title", "title"),
            ("description", "description"),
            // No content was saved previous, so we'll use the description as a stopgap
            // and recrawl stuff
            ("content", "content"),
            ("url", "url"),
        ] {
            let new_field = new_schema.get_field(new_field).unwrap();
            let old_value = old_doc
                .get_first(old_schema.get_field(old_field).unwrap())
                .unwrap()
                .as_text()
                .unwrap();

            new_doc.add_text(new_field, old_value);
        }

        if let Some(tag_list) = tags {
            let new_field = new_schema.get_field("tags").unwrap();
            for tag_id in tag_list {
                new_doc.add_u64(new_field, *tag_id);
            }
        }

        new_doc
    }
}

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20230112_000001_migrate_search_schema"
    }
}

fn get_by_id(id_field: Field, reader: &IndexReader, doc_id: &str) -> Option<Document> {
    let searcher = reader.searcher();

    let query = TermQuery::new(
        Term::from_field_text(id_field, doc_id),
        IndexRecordOption::Basic,
    );

    let res = searcher
        .search(&query, &TopDocs::with_limit(1))
        .expect("Unable to execute query");

    if res.is_empty() {
        return None;
    }

    let (_, doc_address) = res.first().expect("No results in search");
    if let Ok(doc) = searcher.doc(*doc_address) {
        return Some(doc);
    }
    None
}

fn build_tag_map(all_tags: &[QueryResult]) -> HashMap<i64, Vec<u64>> {
    let mut tag_map: HashMap<i64, Vec<u64>> = HashMap::new();

    let tuples = all_tags
        .iter()
        .map(|row| {
            let doc_id: i64 = row.try_get::<i64>("", "indexed_document_id").unwrap();
            let tag_id: i64 = row.try_get::<i64>("", "tag_id").unwrap();
            (doc_id, tag_id)
        })
        .collect::<Vec<(i64, i64)>>();

    for (k, v) in tuples {
        tag_map.entry(k).or_insert_with(Vec::new).push(v as u64);
    }

    tag_map
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "CREATE INDEX IF NOT EXISTS `idx-document_tag-indexed_document_id` ON `document_tag` (`indexed_document_id`);"
                    .to_string(),
            ))
            .await?;

        let result = manager
            .get_connection()
            .query_all(Statement::from_string(
                manager.get_database_backend(),
                "SELECT id, doc_id, url FROM indexed_document".to_owned(),
            ))
            .await?;

        let tags_result = manager
            .get_connection()
            .query_all(Statement::from_string(
                manager.get_database_backend(),
                "SELECT indexed_document_id, tag_id FROM document_tag".to_owned(),
            ))
            .await?;

        let tag_map = build_tag_map(&tags_result);

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

        let old_schema = v2::mapping_to_schema(&self.before_schema());
        let new_schema = v3::mapping_to_schema(&self.after_schema());
        let old_reader_res = self.before_reader(&old_index_path);
        if let Err(err) = old_reader_res {
            // Potentially already migrated?
            println!("Error opening index: {err}");
            return Ok(());
        }
        let old_reader = old_reader_res.expect("Unable to open index for migration");

        let mut new_writer = self.after_writer(&new_index_path);

        let now = Instant::now();
        let old_id_field = old_schema.get_field("id").unwrap();

        let _errs = result
            .par_iter()
            .filter_map(|row| {
                let doc_id: String = row.try_get::<String>("", "doc_id").unwrap();
                let row_id: i64 = row.try_get::<i64>("", "id").unwrap();

                let tags = tag_map.get(&row_id);

                let doc = get_by_id(old_id_field, &old_reader, &doc_id);
                if let Some(old_doc) = doc {
                    if let Err(e) = new_writer.add_document(self.migrate_document(
                        &doc_id,
                        old_doc,
                        &old_schema,
                        &new_schema,
                        tags,
                    )) {
                        return Some(DbErr::Custom(format!("Unable to migrate doc: {e}")));
                    }
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
