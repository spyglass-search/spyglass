use crate::sea_orm::Statement;
use entities::schema::{DocFields, SearchDocument};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

use shared::config::Config;
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::TermQuery;
use tantivy::schema::IndexRecordOption;
use tantivy::{Document, Index, IndexReader, IndexWriter, ReloadPolicy, Term};
use url::Url;

pub struct Migration;

impl Migration {
    fn fix_url(&self, url: &str) -> Option<String> {
        if let Ok(mut parsed) = Url::parse(url) {
            // Switch host to localhost so that we can use `to_file_path`
            let _ = parsed.set_host(Some("localhost"));

            // If we're on Windows, fix the path bug we found where the `ignore`
            // overescapes windows paths.
            if cfg!(target_os = "windows") {
                if let Ok(path_str) = parsed.to_file_path() {
                    let path_str = path_str.display().to_string();
                    parsed.set_path(&path_str.replace("\\\\", "\\"));
                }
            }

            return Some(parsed.to_string());
        }

        None
    }

    fn open_index(&self) -> (IndexWriter, IndexReader) {
        let config = Config::new();
        let schema = DocFields::as_schema();

        let dir = MmapDirectory::open(config.index_dir()).expect("Unable to create MmapDirectory");
        let index = Index::open_or_create(dir, schema).expect("Unable to open / create directory");

        let writer = index
            .writer(50_000_000)
            .expect("Unable to create index_writer");

        // For a search server you will typically create on reader for the entire
        // lifetime of your program.
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommit)
            .try_into()
            .expect("Unable to create reader");

        (writer, reader)
    }

    fn update_index(
        &self,
        writer: &IndexWriter,
        reader: &IndexReader,
        doc_id: &str,
        updated_url: &str,
    ) {
        let fields = DocFields::as_fields();

        let searcher = reader.searcher();
        let query = TermQuery::new(
            Term::from_field_text(fields.id, doc_id),
            IndexRecordOption::Basic,
        );

        let res = searcher
            .search(&query, &TopDocs::with_limit(1))
            .map_or(Vec::new(), |x| x)
            .pop();

        let doc = if let Some((_, doc_address)) = res {
            searcher.doc(doc_address).ok()
        } else {
            None
        };

        // Doc exists! Lets remove it and update it with the new URL
        if let Some(doc) = doc {
            // Remove the old one
            writer.delete_term(Term::from_field_text(fields.id, doc_id));
            // Re-add the document w/ the updated domain & url
            let mut new_doc = Document::default();
            new_doc.add_text(fields.id, doc_id);
            new_doc.add_text(fields.domain, "localhost");
            new_doc.add_text(fields.url, updated_url);
            // Everything else stays the same
            new_doc.add_text(
                fields.content,
                doc.get_first(fields.content).unwrap().as_text().unwrap(),
            );
            new_doc.add_text(
                fields.description,
                doc.get_first(fields.description)
                    .unwrap()
                    .as_text()
                    .unwrap(),
            );
            new_doc.add_text(
                fields.title,
                doc.get_first(fields.title).unwrap().as_text().unwrap(),
            );

            let _ = writer.add_document(new_doc);
        }
    }
}

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20221115_000001_local_file_pathfix"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let (mut iwriter, ireader) = self.open_index();

        println!("Updating crawl_queue");
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "UPDATE crawl_queue SET status = 'Queued' where status = '''Queued'''".to_string(),
            ))
            .await?;

        let queued = manager
            .get_connection()
            .query_all(Statement::from_string(
                manager.get_database_backend(),
                "SELECT id, url FROM crawl_queue WHERE url like 'file://%'".to_owned(),
            ))
            .await?;

        for row in &queued {
            let id = row.try_get::<i64>("", "id");
            let url = row.try_get::<String>("", "url");

            if let (Ok(id), Ok(url)) = (id, url) {
                if let Some(updated_url) = self.fix_url(&url) {
                    manager
                        .get_connection()
                        .execute(Statement::from_sql_and_values(
                            manager.get_database_backend(),
                            "UPDATE crawl_queue SET domain = 'localhost', url = ? WHERE id = ?",
                            vec![id.into(), updated_url.into()],
                        ))
                        .await?;
                }
            }
        }

        let docs = manager
            .get_connection()
            .query_all(Statement::from_string(
                manager.get_database_backend(),
                "SELECT id, doc_id, url FROM indexed_document WHERE url like 'file://%'".to_owned(),
            ))
            .await?;

        // No docs yet, nothing to migrate.
        if docs.is_empty() {
            return Ok(());
        }

        for row in &docs {
            let id = row.try_get::<i64>("", "id");
            let doc_id = row.try_get::<String>("", "doc_id");
            let url = row.try_get::<String>("", "url");

            if let (Ok(id), Ok(doc_id), Ok(url)) = (id, doc_id, url) {
                if let Some(updated_url) = self.fix_url(&url) {
                    // Update the document in the index
                    self.update_index(&iwriter, &ireader, &doc_id, &updated_url);

                    // Update document in the db
                    manager.get_connection()
                        .execute(Statement::from_sql_and_values(
                            manager.get_database_backend(),
                            "UPDATE indexed_document SET domain = 'localhost', open_url = ?, url = ? AND openWHERE id = ?",
                            vec![id.into(), updated_url.clone().into(), updated_url.into()],
                        ))
                        .await?;
                }
            }
        }

        if let Err(err) = iwriter.commit() {
            return Err(DbErr::Custom(format!(
                "Unable to save changes to index: {}",
                err
            )));
        }

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
