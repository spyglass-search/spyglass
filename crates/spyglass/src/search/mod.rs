use std::fmt::{Debug, Error, Formatter};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use regex::RegexSetBuilder;
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::TermQuery;
use tantivy::{schema::*, DocAddress, DocId, SegmentReader};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy};
use uuid::Uuid;

use crate::search::query::build_query;
use crate::search::utils::ff_to_string;
use entities::schema::{DocFields, SearchDocument};
use entities::sea_orm::DatabaseConnection;
use spyglass_plugin::SearchFilter;

pub mod lens;
mod query;
mod utils;

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

impl Searcher {
    /// Delete document w/ `doc_id` from index
    pub fn delete(writer: &mut IndexWriter, doc_id: &str) -> anyhow::Result<()> {
        let fields = DocFields::as_fields();
        writer.delete_term(Term::from_field_text(fields.id, doc_id));
        Ok(())
    }

    /// Get document with `doc_id` from index.
    pub fn get_by_id(reader: &IndexReader, doc_id: &str) -> Option<Document> {
        let fields = DocFields::as_fields();
        let searcher = reader.searcher();

        let query = TermQuery::new(
            Term::from_field_text(fields.id, doc_id),
            IndexRecordOption::Basic,
        );

        let res = searcher
            .search(&query, &TopDocs::with_limit(1))
            .map_or(Vec::new(), |x| x);

        if res.is_empty() {
            return None;
        }

        if let Some((_, doc_address)) = res.first() {
            if let Ok(doc) = searcher.doc(*doc_address) {
                return Some(doc);
            }
        }

        None
    }

    /// Constructs a new Searcher object w/ the index @ `index_path`
    pub fn with_index(index_path: &IndexPath) -> Self {
        let schema = DocFields::as_schema();
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
        // Save to a cache?
        _raw: &str,
    ) -> tantivy::Result<String> {
        let fields = DocFields::as_fields();

        let doc_id = Uuid::new_v4().as_hyphenated().to_string();
        let mut doc = Document::default();
        doc.add_text(fields.content, content);
        doc.add_text(fields.description, description);
        doc.add_text(fields.domain, domain);
        doc.add_text(fields.id, &doc_id);
        doc.add_text(fields.title, title);
        doc.add_text(fields.url, url);
        writer.add_document(doc)?;

        Ok(doc_id)
    }

    pub async fn search_with_lens(
        _db: DatabaseConnection,
        applied_lenses: &Vec<SearchFilter>,
        reader: &IndexReader,
        query_string: &str,
    ) -> Vec<SearchResult> {
        let start_timer = Instant::now();

        let fields = DocFields::as_fields();
        let searcher = reader.searcher();

        let query = build_query(fields.clone(), query_string);

        let mut patterns = Vec::new();
        for filter in applied_lenses {
            match filter {
                SearchFilter::URLRegex(regex) => patterns.push(regex),
                SearchFilter::None => {}
            }
        }

        let regex = RegexSetBuilder::new(patterns)
            // Allow some beefy regexes
            .size_limit(100_000_000)
            .build()
            .expect("Unable to build regexset");

        let collector =
            TopDocs::with_limit(5).tweak_score(move |segment_reader: &SegmentReader| {
                let regex = regex.clone();
                let fields = fields.clone();

                let inverted_index = segment_reader
                    .inverted_index(fields.url)
                    .expect("Failed to get inverted index for segment");

                let id_reader = segment_reader
                    .fast_fields()
                    .u64s(fields.id)
                    .expect("Unable to get fast field for doc_id");

                let url_reader = segment_reader
                    .fast_fields()
                    .u64s(fields.url)
                    .expect("Unable to get fast field for URL");

                // We can now define our actual scoring function
                move |doc: DocId, original_score: Score| {
                    let inverted_index = inverted_index.clone();
                    let terms = inverted_index.terms();

                    let _id = ff_to_string(doc, &id_reader, terms);
                    let url = ff_to_string(doc, &url_reader, terms);

                    if let Some(url) = url {
                        if regex.is_empty() || regex.is_match(&url) {
                            original_score * 1.0
                        } else {
                            -1.0
                        }
                    } else {
                        // blank URL? that seems like an error somewhere.
                        -1.0
                    }
                }
            });

        let top_docs = searcher
            .search(&query, &collector)
            .expect("Unable to execute query");

        log::info!(
            "query `{}` returned {} results from {} docs in {} ms",
            query_string,
            top_docs.len(),
            searcher.num_docs(),
            Instant::now().duration_since(start_timer).as_millis()
        );

        top_docs
            .into_iter()
            // Filter out negative scores
            .filter(|(score, _)| *score >= 0.0)
            .collect()
    }
}

#[cfg(test)]
mod test {
    use crate::search::{IndexPath, Searcher};
    use entities::models::create_connection;
    use shared::config::{Config, LensConfig};

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

    #[tokio::test]
    pub async fn test_basic_lense_search() {
        let db = create_connection(&Config::default(), true).await.unwrap();
        let lens = LensConfig {
            name: "wiki".to_string(),
            domains: vec!["en.wikipedia.org".to_string()],
            urls: Vec::new(),
            ..Default::default()
        };

        let applied_lens = vec![lens.clone()];
        let mut searcher = Searcher::with_index(&IndexPath::Memory);
        _build_test_index(&mut searcher);

        let query = "salinas";
        let results = Searcher::search_with_lens(db, &applied_lens, &searcher.reader, query).await;
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    pub async fn test_url_lens_search() {
        let db = create_connection(&Config::default(), true).await.unwrap();

        let lens = LensConfig {
            name: "wiki".to_string(),
            domains: Vec::new(),
            urls: vec!["https://en.wikipedia.org/mice".to_string()],
            ..Default::default()
        };

        let applied_lens = vec![lens.clone()];
        let mut searcher = Searcher::with_index(&IndexPath::Memory);
        _build_test_index(&mut searcher);

        let query = "salinas";
        let results = Searcher::search_with_lens(db, &applied_lens, &searcher.reader, query).await;
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    pub async fn test_singular_url_lens_search() {
        let db = create_connection(&Config::default(), true).await.unwrap();

        let lens = LensConfig {
            name: "wiki".to_string(),
            domains: Vec::new(),
            urls: vec!["https://en.wikipedia.org/mice$".to_string()],
            ..Default::default()
        };

        let applied_lens = vec![lens.clone()];
        lenses.insert("wiki".to_string(), lens.clone());

        let mut searcher = Searcher::with_index(&IndexPath::Memory);
        _build_test_index(&mut searcher);

        let query = "salinas";
        let results = Searcher::search_with_lens(db, &applied_lens, &searcher.reader, query).await;
        assert_eq!(results.len(), 0);
    }
}
