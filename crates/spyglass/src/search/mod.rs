#![allow(dead_code)]
use std::collections::HashMap;
use std::fmt::{Debug, Error, Formatter};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::{Occur, Query, QueryParser, TermQuery};
use tantivy::{schema::*, DocAddress};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy};
use uuid::Uuid;

pub mod lens;
mod query;
use crate::search::query::build_query;
use shared::config::Lens;

type Score = f32;
type SearchResult = (Score, DocAddress);

pub enum IndexPath {
    // Directory
    LocalPath(PathBuf),
    // In memory index for testing purposes.
    Memory,
}

#[derive(Clone)]
pub struct Searcher {
    pub index: Index,
    pub reader: IndexReader,
    pub writer: Arc<Mutex<IndexWriter>>,
}

impl Debug for Searcher {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.debug_struct("Searcher")
            .field("index", &self.index)
            .finish()
    }
}

pub struct DocFields {
    pub id: Field,
    pub domain: Field,
    pub content: Field,
    pub description: Field,
    pub title: Field,
    pub url: Field,
    pub raw: Field,
}

type QueryVec = Vec<(Occur, Box<dyn Query>)>;

impl Searcher {
    pub fn schema() -> Schema {
        let mut schema_builder = Schema::builder();
        // Our first field is title. We want:
        // - full-text search and
        // - to retrieve the document after the search
        //
        // TEXT:    Means the field should be tokenized and indexed, along with its term
        //          frequency and term positions.
        // STRING:  Means the field will be untokenized and indexed unlike above
        //
        // STORED:  Means that the field will also be saved in a compressed, row oriented
        //          key-value store. This store is useful to reconstruct the documents that
        //          were selected during the search phase.
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

    pub fn delete(writer: &mut IndexWriter, id: &str) -> anyhow::Result<()> {
        let fields = Searcher::doc_fields();
        writer.delete_term(Term::from_field_text(fields.id, id));
        Ok(())
    }

    pub fn doc_fields() -> DocFields {
        let schema = Searcher::schema();

        DocFields {
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

    pub fn get_by_id(reader: &IndexReader, doc_id: &str) -> Option<Document> {
        let fields = Searcher::doc_fields();
        let searcher = reader.searcher();

        let query = TermQuery::new(
            Term::from_field_text(fields.id, doc_id),
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

    pub fn with_index(index_path: &IndexPath) -> Self {
        let schema = Searcher::schema();
        let index = match index_path {
            IndexPath::LocalPath(path) => {
                let dir = MmapDirectory::open(path).expect("Unable to mmap search index");
                Index::open_or_create(dir, schema).expect("Unable to open search index")
            }
            IndexPath::Memory => Index::create_in_ram(schema),
        };

        // Should only be one writer at a time. This single IndexWriter is already
        // multithreaded.
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

        Searcher {
            index,
            reader,
            writer: Arc::new(Mutex::new(writer)),
        }
    }

    pub fn add_document(
        writer: &mut IndexWriter,
        title: &str,
        description: &str,
        domain: &str,
        url: &str,
        content: &str,
        raw: &str,
    ) -> tantivy::Result<String> {
        let fields = Searcher::doc_fields();

        let doc_id = Uuid::new_v4().as_hyphenated().to_string();
        let mut doc = Document::default();
        doc.add_text(fields.content, content);
        doc.add_text(fields.description, description);
        doc.add_text(fields.domain, domain);
        doc.add_text(fields.id, &doc_id);
        doc.add_text(fields.raw, raw);
        doc.add_text(fields.title, title);
        doc.add_text(fields.url, url);
        writer.add_document(doc)?;

        Ok(doc_id)
    }

    pub fn search(index: &Index, reader: &IndexReader, query_string: &str) -> Vec<SearchResult> {
        let fields = Searcher::doc_fields();
        let searcher = reader.searcher();

        let query_parser = QueryParser::for_index(index, vec![fields.title, fields.content]);

        let query = query_parser
            .parse_query(query_string)
            .expect("Unable to parse query");

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(10))
            .expect("Unable to execute query");

        log::info!(
            "query `{}` returned {} results from {} docs",
            query_string,
            top_docs.len(),
            searcher.num_docs(),
        );
        top_docs.into_iter().collect()
    }

    pub fn search_with_lens(
        lenses: &HashMap<String, Lens>,
        reader: &IndexReader,
        applied_lens: &[String],
        query_string: &str,
    ) -> Vec<SearchResult> {
        let fields = Searcher::doc_fields();
        let searcher = reader.searcher();

        let query = build_query(fields, lenses, applied_lens, query_string);

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(5))
            .expect("Unable to execute query");

        log::info!(
            "query `{}` returned {} results from {} docs",
            query_string,
            top_docs.len(),
            searcher.num_docs(),
        );

        top_docs.into_iter().collect()
    }
}

#[cfg(test)]
mod test {
    use crate::search::{IndexPath, Searcher};
    use shared::config::Lens;
    use std::collections::HashMap;

    fn _build_test_index(searcher: &mut Searcher) {
        let writer = &mut searcher.writer.lock().unwrap();
        Searcher::add_document(
            writer,
            "Of Mice and Men",
            "Of Mice and Men passage",
            "example.com",
            "https://example.com/mice_and_men",
            "A few miles south of Soledad, the Salinas River drops in close to the hillside
            bank and runs deep and green. The water is warm too, for it has slipped twinkling
            over the yellow sands in the sunlight before reaching the narrow pool. On one
            side of the river the golden foothill slopes curve up to the strong and rocky
            Gabilan Mountains, but on the valley side the water is lined with trees—willows
            fresh and green with every spring, carrying in their lower leaf junctures the
            debris of the winter’s flooding; and sycamores with mottled, white, recumbent
            limbs and branches that arch over the pool",
            "",
        )
        .expect("Unable to add doc");

        Searcher::add_document(
            writer,
            "Of Mice and Men",
            "Of Mice and Men passage",
            "en.wikipedia.org",
            "https://en.wikipedia.org/mice_and_men",
            "A few miles south of Soledad, the Salinas River drops in close to the hillside
            bank and runs deep and green. The water is warm too, for it has slipped twinkling
            over the yellow sands in the sunlight before reaching the narrow pool. On one
            side of the river the golden foothill slopes curve up to the strong and rocky
            Gabilan Mountains, but on the valley side the water is lined with trees—willows
            fresh and green with every spring, carrying in their lower leaf junctures the
            debris of the winter’s flooding; and sycamores with mottled, white, recumbent
            limbs and branches that arch over the pool",
            "",
        )
        .expect("Unable to add doc");

        Searcher::add_document(
            writer,
            "Of Cheese and Crackers",
            "Of Cheese and Crackers Passage",
            "en.wikipedia.org",
            "https://en.wikipedia.org/cheese_and_crackers",
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Nulla
            tellus tortor, varius sit amet fermentum a, finibus porttitor erat. Proin
            suscipit, dui ac posuere vulputate, justo est faucibus est, a bibendum
            nulla nulla sed elit. Vivamus et libero a tortor ultricies feugiat in vel
            eros. Donec rhoncus mauris libero, et imperdiet neque sagittis sed. Nulla
            ac volutpat massa. Vivamus sed imperdiet est, id pretium ex. Praesent suscipit
            mattis ipsum, a lacinia nunc semper vitae.",
            "",
        )
        .expect("Unable to add doc");

        Searcher::add_document(
            writer,
            "Frankenstein: The Modern Prometheus",
            "A passage from Frankenstein",
            "monster.com",
            "https://example.com/frankenstein",
            "You will rejoice to hear that no disaster has accompanied the commencement of an
             enterprise which you have regarded with such evil forebodings.  I arrived here
             yesterday, and my first task is to assure my dear sister of my welfare and
             increasing confidence in the success of my undertaking.",
            "",
        )
        .expect("Unable to add doc");

        let res = writer.commit();
        if let Err(err) = res {
            println!("{:?}", err);
        }

        // add a small delay so that the documents can be properly committed
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }

    #[test]
    pub fn test_indexer() {
        let mut searcher = Searcher::with_index(&IndexPath::Memory);
        _build_test_index(&mut searcher);

        let results = Searcher::search(&searcher.index, &searcher.reader, "gabilan mountains");
        assert_eq!(results.len(), 2);
    }

    #[test]
    pub fn test_basic_lense_search() {
        let lens = Lens {
            name: "wiki".to_string(),
            domains: vec!["en.wikipedia.org".to_string()],
            urls: Vec::new(),
            ..Default::default()
        };

        let applied_lens = vec!["wiki".to_string()];

        let mut lenses = HashMap::new();
        lenses.insert("wiki".to_string(), lens.clone());

        let mut searcher = Searcher::with_index(&IndexPath::Memory);
        _build_test_index(&mut searcher);

        let query = "salinas";
        let results = Searcher::search_with_lens(&lenses, &searcher.reader, &applied_lens, query);
        assert_eq!(results.len(), 1);
    }

    #[test]
    pub fn test_url_lens_search() {
        let lens = Lens {
            name: "wiki".to_string(),
            domains: Vec::new(),
            urls: vec!["https://en.wikipedia.org/mice".to_string()],
            ..Default::default()
        };

        let applied_lens = vec!["wiki".to_string()];

        let mut lenses = HashMap::new();
        lenses.insert("wiki".to_string(), lens.clone());

        let mut searcher = Searcher::with_index(&IndexPath::Memory);
        _build_test_index(&mut searcher);

        let query = "salinas";
        let results = Searcher::search_with_lens(&lenses, &searcher.reader, &applied_lens, query);
        assert_eq!(results.len(), 1);
    }

    #[test]
    pub fn test_singular_url_lens_search() {
        let lens = Lens {
            name: "wiki".to_string(),
            domains: Vec::new(),
            urls: vec!["https://en.wikipedia.org/mice$".to_string()],
            ..Default::default()
        };

        let applied_lens = vec!["wiki".to_string()];

        let mut lenses = HashMap::new();
        lenses.insert("wiki".to_string(), lens.clone());

        let mut searcher = Searcher::with_index(&IndexPath::Memory);
        _build_test_index(&mut searcher);

        let query = "salinas";
        let results = Searcher::search_with_lens(&lenses, &searcher.reader, &applied_lens, query);
        assert_eq!(results.len(), 0);
    }
}
