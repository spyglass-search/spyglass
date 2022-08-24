use tantivy::schema::*;

pub struct DocFields {
    pub id: Field,
    pub domain: Field,
    pub content: Field,
    pub description: Field,
    pub title: Field,
    pub url: Field,
    pub raw: Field,
}

impl DocFields {
    pub fn as_schema() -> Schema {
        // TEXT:    Means the field should be tokenized and indexed, along with its term
        //          frequency and term positions.
        // STRING:  Means the field will be untokenized and indexed unlike above
        //
        // STORED:  Means that the field will also be saved in a compressed, row oriented
        //          key-value store. This store is useful to reconstruct the documents that
        //          were selected during the search phase.
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("id", STRING | STORED);
        schema_builder.add_text_field("domain", STRING | STORED);

        schema_builder.add_text_field("title", TEXT | STORED);
        schema_builder.add_text_field("description", TEXT | STORED);
        schema_builder.add_text_field("url", STRING | STORED);
        // Indexed but don't store for retreival
        schema_builder.add_text_field("content", TEXT);
        // Stored but not indexed
        schema_builder.add_text_field("raw", STORED);

        schema_builder.build()
    }

    pub fn as_fields() -> Self {
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
            raw: schema.get_field("raw").expect("No raw in schema"),
        }
    }
}
