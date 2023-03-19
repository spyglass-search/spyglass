use std::{collections::HashMap, path::PathBuf};

use tantivy_18::{
    collector::TopDocs, directory::MmapDirectory, query::TermQuery, schema::*, Index, IndexReader,
    ReloadPolicy, TantivyError,
};

pub type FieldName = String;
pub struct SchemaMapping {
    pub text_fields: Option<Vec<(FieldName, TextOptions)>>,
    pub unsigned_fields: Option<Vec<(FieldName, NumericOptions)>>,
}

pub trait SearchDocument {
    fn as_field_vec() -> SchemaMapping;

    fn as_schema() -> Schema {
        mapping_to_schema(&Self::as_field_vec())
    }

    fn as_fields() -> Self;
}

pub fn mapping_to_schema(mapping: &SchemaMapping) -> Schema {
    let mut schema_builder = Schema::builder();
    if let Some(fields) = &mapping.text_fields {
        for (name, opts) in fields {
            schema_builder.add_text_field(name, opts.clone());
        }
    }

    if let Some(fields) = &mapping.unsigned_fields {
        for (name, opts) in fields {
            schema_builder.add_u64_field(name, opts.clone());
        }
    }
    schema_builder.build()
}

#[derive(Clone)]
pub struct DocFields {
    pub id: Field,
    pub domain: Field,
    pub content: Field,
    pub description: Field,
    pub title: Field,
    pub url: Field,
    pub tags: Field,
}

impl SearchDocument for DocFields {
    fn as_field_vec() -> SchemaMapping {
        // FAST:    Fast fields can be random-accessed rapidly. Use this for fields useful
        //          for scoring, filtering, or collection.
        // TEXT:    Means the field should be tokenized and indexed, along with its term
        //          frequency and term positions.
        // STRING:  Means the field will be untokenized and indexed unlike above
        //
        // STORED:  Means that the field will also be saved in a compressed, row oriented
        //          key-value store. This store is useful to reconstruct the documents that
        //          were selected during the search phase.
        SchemaMapping {
            text_fields: Some(vec![
                // Used to reference this document
                ("id".into(), STRING | STORED | FAST),
                // Document contents
                ("domain".into(), STRING | STORED | FAST),
                ("title".into(), TEXT | STORED | FAST),
                // Used for display purposes
                ("description".into(), TEXT | STORED),
                ("url".into(), STRING | STORED | FAST),
                // Indexed
                ("content".into(), TEXT | STORED),
            ]),
            unsigned_fields: Some(vec![(
                "tags".into(),
                NumericOptions::default()
                    .set_fast(Cardinality::MultiValues)
                    .set_indexed()
                    .set_stored(),
            )]),
        }
    }

    fn as_fields() -> Self {
        let schema = Self::as_schema();
        Self {
            id: schema.get_field("id").expect("No id in schema"),
            domain: schema.get_field("domain").expect("No domain in schema"),
            content: schema.get_field("content").expect("No content in schema"),
            description: schema
                .get_field("description")
                .expect("No description in schema"),
            title: schema.get_field("title").expect("No title in schema"),
            url: schema.get_field("url").expect("No url in schema"),
            tags: schema.get_field("tags").expect("No tags in schema"),
        }
    }
}

/// Represents the Third version of the index schema. Note that the
/// import is for tantivy 0.18 since version V3 requires version 0.18
pub struct SchemaReader {
    reader: Option<IndexReader>,
}

impl SchemaReader {
    pub fn new(path: &PathBuf) -> Self {
        match reader(path) {
            Ok(reader) => Self {
                reader: Some(reader),
            },
            Err(err) => {
                log::error!("Error initializing reader for V3 schema {:?}", err);
                Self { reader: None }
            }
        }
    }

    /// Indicates if the reader was able to initialize
    pub fn has_reader(&self) -> bool {
        self.reader.is_some()
    }

    /// Returns a map of all text values for the specified document, the key is
    /// the field name and the value is the field value.
    pub fn get_txt_values(&self, doc_id: &str) -> HashMap<String, String> {
        let txt_fields = DocFields::as_field_vec().text_fields.unwrap();
        let mut map = HashMap::new();

        if let Some(reader) = &self.reader {
            let searcher = reader.searcher();
            let schema = searcher.schema();
            let id_field = schema.get_field("id").unwrap();

            if let Some(doc) = get_by_id(id_field, reader, doc_id) {
                for (name, _) in txt_fields {
                    let old_value = doc
                        .get_first(schema.get_field(name.as_str()).unwrap())
                        .unwrap()
                        .as_text()
                        .unwrap();

                    map.insert(name, old_value.into());
                }
            }
        }
        map
    }

    /// Returns a map of all unsigned values for the specified document (as of this version this is only a list of tags),
    /// the key is the field name and the value is the field value.
    pub fn get_unsigned_fields(&self, doc_id: &str) -> HashMap<String, Vec<u64>> {
        let unsigned_fields = DocFields::as_field_vec().unsigned_fields.unwrap();
        let mut map = HashMap::new();

        if let Some(reader) = &self.reader {
            let searcher = reader.searcher();
            let schema = searcher.schema();
            let id_field = schema.get_field("id").unwrap();

            if let Some(doc) = get_by_id(id_field, reader, doc_id) {
                for (name, _) in unsigned_fields {
                    let old_value = doc
                        .get_all(schema.get_field(name.as_str()).unwrap())
                        .filter_map(|val| val.as_u64())
                        .collect::<Vec<u64>>();

                    map.insert(name, old_value);
                }
            }
        }

        map
    }
}

/// Helper method used to get the document by id
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

/// Helper to build a reader that is compatible with schema version 3
pub fn reader(path: &PathBuf) -> Result<IndexReader, TantivyError> {
    let dir = MmapDirectory::open(path).expect("Unable to mmap search index");
    let mapping = DocFields::as_field_vec();
    let index = Index::open_or_create(dir, mapping_to_schema(&mapping))?;

    index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
}
