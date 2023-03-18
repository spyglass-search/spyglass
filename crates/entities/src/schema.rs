use tantivy::schema::*;

pub type FieldName = String;

/// The current schema version
pub const SCHEMA_VERSION: &str = "4";
pub struct SchemaMapping {
    pub text_fields: Option<Vec<(FieldName, TextOptions)>>,
    pub date_fields: Option<Vec<(FieldName, DateOptions)>>,
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

    if let Some(fields) = &mapping.date_fields {
        for (name, opts) in fields {
            schema_builder.add_date_field(name, opts.clone());
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
    pub published: Field,
    pub lastmodified: Field,
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
            date_fields: Some(vec![
                (
                    "published".into(),
                    DateOptions::default()
                        .set_precision(DatePrecision::Microseconds)
                        .set_fast(Cardinality::SingleValue)
                        .set_indexed()
                        .set_stored(),
                ),
                (
                    "lastmodified".into(),
                    DateOptions::default()
                        .set_precision(DatePrecision::Microseconds)
                        .set_fast(Cardinality::SingleValue)
                        .set_indexed()
                        .set_stored(),
                ),
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
            published: schema
                .get_field("published")
                .expect("No published date in schema"),
            lastmodified: schema
                .get_field("lastmodified")
                .expect("No last modified date in schema"),
        }
    }
}
