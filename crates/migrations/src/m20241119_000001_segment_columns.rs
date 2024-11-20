use sea_orm::Statement;
use sea_orm_migration::prelude::*;
use shared::config::Config;
use spyglass_searcher::schema::DocFields;
use spyglass_searcher::schema::SearchDocument;
use tantivy::collector::TopDocs;
use tantivy::query::TermQuery;
use tantivy::schema::Field;
use tantivy::schema::IndexRecordOption;
use tantivy::Document;
use tantivy::Term;
use tantivy::{directory::MmapDirectory, IndexReader, IndexWriter, ReloadPolicy};
use tokenizers::decoders::metaspace::PrependScheme;
use tokenizers::pre_tokenizers::sequence::Sequence;
use tokenizers::PreTokenizerWrapper;
use tokenizers::Tokenizer;

const MAX_TOKENS: usize = 2048;

const TOKENIZER_CONFIG: &[u8] = include_bytes!("./config/tokenizer.json");

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum VecToIndexed {
    #[iden = "vec_to_indexed"]
    Table,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if let Ok(false) = manager.has_column("vec_to_indexed", "segment_start").await {
            manager
                .alter_table(
                    Table::alter()
                        .table(VecToIndexed::Table)
                        .add_column(ColumnDef::new(Alias::new("segment_start")).big_integer())
                        .to_owned(),
                )
                .await?;
        }

        if let Ok(false) = manager.has_column("vec_to_indexed", "segment_end").await {
            manager
                .alter_table(
                    Table::alter()
                        .table(VecToIndexed::Table)
                        .add_column(ColumnDef::new(Alias::new("segment_end")).big_integer())
                        .to_owned(),
                )
                .await?;
        }

        let results = manager
            .get_connection()
            .query_all(Statement::from_string(
                manager.get_database_backend(),
                r#"SELECT vec_to_indexed.id, vec_to_indexed.indexed_id, indexed_document.doc_id, vec_to_indexed.segment_start FROM vec_to_indexed
                 left join indexed_document on indexed_document.id = vec_to_indexed.indexed_id
                 group by vec_to_indexed.indexed_id
                 "#.to_owned(),
            ))
            .await?;

        let tokenizer = load_tokenizer();
        for result in results {
            let id: Result<i64, DbErr> = result.try_get("", "id");
            let indexed_id: Result<i64, DbErr> = result.try_get("", "indexed_id");
            let segment_start: Result<i64, DbErr> = result.try_get("", "segment_start");
            let doc_id: Result<String, DbErr> = result.try_get("", "doc_id");

            if segment_start.is_err() {
                if let (Ok(id), Ok(indexed_id), Ok(doc_id)) = (id, indexed_id, doc_id) {
                    let (_writer, reader) = open_index();
                    let schema = DocFields::as_schema();
                    let id_field = schema.get_field("id").unwrap();
                    let content_field = schema.get_field("content").unwrap();

                    if let Some(doc) = get_by_id(id_field, &reader, &doc_id) {
                        let content = doc.get_first(content_field);
                        if let Some(content) = content {
                            if let Some(content) = content.as_text() {
                                calc_update_length(id, indexed_id, content, manager, &tokenizer)
                                    .await?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

async fn calc_update_length(
    id: i64,
    indexed_id: i64,
    content: &str,
    manager: &SchemaManager<'_>,
    tokenizer: &Tokenizer,
) -> Result<(), DbErr> {
    let content_len = content.trim().len();
    let token_len = tokenizer
        .encode(content.trim(), false)
        .map(|encoding| encoding.len())
        .unwrap_or(content_len);

    if token_len > MAX_TOKENS {
        let results = manager
            .get_connection()
            .query_all(Statement::from_sql_and_values(
                manager.get_database_backend(),
                r#"SELECT vec_to_indexed.id FROM vec_to_indexed
                left join indexed_document on indexed_document.id = vec_to_indexed.indexed_id
                where vec_to_indexed.indexed_id = $1
                order by vec_to_indexed.id ASC
                "#
                .to_owned(),
                vec![indexed_id.into()],
            ))
            .await?;

        let mut index_list = Vec::new();
        for result in results {
            let id: Result<i64, DbErr> = result.try_get("", "id");
            if let Ok(id) = id {
                index_list.push(id);
            }
        }
        index_list.reverse();

        let segment_count = token_len.div_ceil(MAX_TOKENS);
        let char_per_segment = content_len.div_ceil(segment_count);

        let mut i = 0;
        while i < content_len {
            let start = i as i64;
            let mut end: i64 = (i + char_per_segment - 1) as i64;
            end = end.min((content_len - 1) as i64);

            let vec_id = index_list.pop().unwrap();
            let statement = Statement::from_sql_and_values(
                manager.get_database_backend(),
                r#"
                    UPDATE vec_to_indexed set segment_start = $1, segment_end = $2
                    WHERE id = $3 AND indexed_id = $4
                "#,
                vec![start.into(), end.into(), vec_id.into(), indexed_id.into()],
            );

            let _ = manager.get_connection().execute(statement).await?;
            i += char_per_segment;
        }
    } else {
        let end = (content.len() - 1) as i64;
        let statement = Statement::from_sql_and_values(
            manager.get_database_backend(),
            r#"
                UPDATE vec_to_indexed set segment_start = 0, segment_end = $1
                WHERE id = $2
            "#,
            vec![end.into(), id.into()],
        );

        let _ = manager.get_connection().execute(statement).await?;
    }

    Ok(())
}

fn open_index() -> (IndexWriter, IndexReader) {
    let config = Config::new();
    let schema = DocFields::as_schema();

    let dir = MmapDirectory::open(config.index_dir()).expect("Unable to create MmapDirectory");
    let index =
        tantivy::Index::open_or_create(dir, schema).expect("Unable to open / create directory");

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

pub fn load_tokenizer() -> Tokenizer {
    // Load tokenizer
    let mut tokenizer = Tokenizer::from_bytes(TOKENIZER_CONFIG).expect("Unable to load Tokenizer");
    // See https://github.com/huggingface/tokenizers/pull/1357
    if let Some(pre_tokenizer) = tokenizer.get_pre_tokenizer() {
        if let PreTokenizerWrapper::Metaspace(m) = pre_tokenizer {
            // We are forced to clone since `Tokenizer` does not have a `get_mut` for `pre_tokenizer`
            let mut m = m.clone();
            m.set_prepend_scheme(PrependScheme::First);
            tokenizer.with_pre_tokenizer(Some(PreTokenizerWrapper::Metaspace(m)));
        } else if let PreTokenizerWrapper::Sequence(s) = pre_tokenizer {
            let pre_tokenizers = s.get_pre_tokenizers();
            // Check if we have a Metaspace pre tokenizer in the sequence
            let has_metaspace = pre_tokenizers
                .iter()
                .any(|t| matches!(t, PreTokenizerWrapper::Metaspace(_)));

            if has_metaspace {
                let mut new_pre_tokenizers = Vec::with_capacity(s.get_pre_tokenizers().len());

                for pre_tokenizer in pre_tokenizers {
                    if let PreTokenizerWrapper::WhitespaceSplit(_) = pre_tokenizer {
                        // Remove WhitespaceSplit
                        // This will be done by the Metaspace pre tokenizer
                        continue;
                    }

                    let mut pre_tokenizer = pre_tokenizer.clone();

                    if let PreTokenizerWrapper::Metaspace(ref mut m) = pre_tokenizer {
                        m.set_prepend_scheme(PrependScheme::First);
                    }
                    new_pre_tokenizers.push(pre_tokenizer);
                }
                tokenizer.with_pre_tokenizer(Some(PreTokenizerWrapper::Sequence(Sequence::new(
                    new_pre_tokenizers,
                ))));
            }
        }
    }

    tokenizer.with_padding(None);
    tokenizer
}
