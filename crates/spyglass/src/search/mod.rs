#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;

use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, Query, QueryParser, TermQuery};
use tantivy::{schema::*, DocAddress};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy};
use uuid::Uuid;

use crate::config::Lens;

type Score = f32;
type SearchResult = (Score, DocAddress);

pub enum IndexPath {
    // Directory
    LocalPath(PathBuf),
    // In memory index for testing purposes.
    Memory,
}

pub struct Searcher {
    pub index: Index,
    pub reader: IndexReader,
    pub writer: IndexWriter,
}

pub struct DocFields {
    pub id: Field,
    pub domain: Field,
    pub content: Field,
    pub description: Field,
    pub title: Field,
    pub url: Field,
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
        schema_builder.add_text_field("url", TEXT | STORED);
        // Indexed but don't store for retreival
        schema_builder.add_text_field("content", TEXT);

        schema_builder.build()
    }

    pub fn delete(writer: &mut IndexWriter, id: &str) -> anyhow::Result<()> {
        let fields = Searcher::doc_fields();
        writer.delete_term(Term::from_field_text(fields.id, id));
        writer.commit()?;

        Ok(())
    }

    pub fn doc_fields() -> DocFields {
        let schema = Searcher::schema();

        DocFields {
            id: schema.get_field("id").unwrap(),
            domain: schema.get_field("domain").unwrap(),
            content: schema.get_field("content").unwrap(),
            description: schema.get_field("description").unwrap(),
            title: schema.get_field("title").unwrap(),
            url: schema.get_field("url").unwrap(),
        }
    }

    pub fn with_index(index_path: &IndexPath) -> Self {
        let schema = Searcher::schema();
        let index = match index_path {
            IndexPath::LocalPath(path) => {
                let dir = MmapDirectory::open(path).unwrap();
                Index::open_or_create(dir, schema).unwrap()
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
            writer,
        }
    }

    pub fn add_document(
        writer: &mut IndexWriter,
        title: &str,
        description: &str,
        domain: &str,
        url: &str,
        content: &str,
    ) -> tantivy::Result<String> {
        let fields = Searcher::doc_fields();

        let doc_id = Uuid::new_v4().to_hyphenated().to_string();
        let mut doc = Document::default();
        doc.add_text(fields.id, &doc_id);
        doc.add_text(fields.domain, domain);
        doc.add_text(fields.content, content);
        doc.add_text(fields.description, description);
        doc.add_text(fields.title, title);
        doc.add_text(fields.url, url);
        writer.add_document(doc)?;

        writer.commit()?;

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
        _index: &Index,
        reader: &IndexReader,
        applied_lens: &[String],
        query_string: &str,
    ) -> Vec<SearchResult> {
        let fields = Searcher::doc_fields();
        let searcher = reader.searcher();

        // Tokenize query string
        let mut terms = Vec::new();

        for term in query_string.split(' ') {
            // remove whitespace
            let term = term.trim();
            terms.push(term);
        }

        log::info!("lenses: {:?}, terms: {:?}", applied_lens, terms);

        let mut lense_queries: QueryVec = Vec::new();
        for lens in applied_lens {
            if lenses.contains_key(lens) {
                let lens = lenses.get(lens).unwrap();
                for domain in &lens.domains {
                    lense_queries.push((
                        Occur::Should,
                        Box::new(TermQuery::new(
                            Term::from_field_text(fields.domain, domain),
                            IndexRecordOption::Basic,
                        )),
                    ));
                }
            }
        }

        let mut term_query: QueryVec = Vec::new();
        for term in terms {
            term_query.push((
                Occur::Should,
                Box::new(TermQuery::new(
                    Term::from_field_text(fields.content, term),
                    IndexRecordOption::Basic,
                )),
                // Box::new(FuzzyTermQuery::new(
                //     Term::from_field_text(fields.content, term),
                //     1,
                //     true,
                // )),
            ))
        }

        let mut nested_query: QueryVec =
            vec![(Occur::Must, Box::new(BooleanQuery::new(term_query)))];
        if !lense_queries.is_empty() {
            nested_query.push((Occur::Must, Box::new(BooleanQuery::new(lense_queries))));
        }

        log::debug!("QUERY: {:?}", nested_query);

        let query = BooleanQuery::new(nested_query);
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
    use crate::config::Lens;
    use crate::search::{IndexPath, Searcher};
    use std::collections::HashMap;

    fn _build_test_index(searcher: &mut Searcher) {
        let writer = &mut searcher.writer;
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
        )
        .expect("Unable to add doc");

        // add a small delay so that the documents can be properly committed
        std::thread::sleep(std::time::Duration::from_millis(100));
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
        };

        let applied_lens = vec!["wiki".to_string()];

        let mut lenses = HashMap::new();
        lenses.insert("wiki".to_string(), lens.clone());

        let mut searcher = Searcher::with_index(&IndexPath::Memory);
        _build_test_index(&mut searcher);

        let query = "::wiki:: salinas";
        let results = Searcher::search_with_lens(
            &lenses,
            &searcher.index,
            &searcher.reader,
            &applied_lens,
            query
        );
        assert_eq!(results.len(), 1);
    }
}
