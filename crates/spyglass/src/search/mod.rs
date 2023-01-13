use std::fmt::{Debug, Error, Formatter};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::TermQuery;
use tantivy::{schema::*, DocAddress};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy};
use uuid::Uuid;

use crate::search::query::build_query;
use crate::state::AppState;
use entities::models::{document_tag, indexed_document};
use entities::schema::{DocFields, SearchDocument};
use entities::sea_orm::{prelude::*, DatabaseConnection};

pub mod grouping;
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

#[derive(Clone)]
pub struct DocumentUpdate<'a> {
    pub doc_id: Option<String>,
    pub title: &'a str,
    pub description: &'a str,
    pub domain: &'a str,
    pub url: &'a str,
    pub content: &'a str,
    pub tags: &'a Option<Vec<u64>>,
}

impl Debug for Searcher {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.debug_struct("Searcher")
            .field("index", &self.index)
            .finish()
    }
}

impl Searcher {
    pub async fn save(state: &AppState) -> anyhow::Result<()> {
        if let Ok(mut writer) = state.index.writer.lock() {
            match writer.commit() {
                Ok(_) => Ok(()),
                Err(err) => Err(anyhow::anyhow!(err.to_string())),
            }
        } else {
            Ok(())
        }
    }

    pub async fn delete_by_id(state: &AppState, doc_id: &str) -> anyhow::Result<()> {
        // Remove from search index, immediately.
        if let Ok(mut writer) = state.index.writer.lock() {
            Searcher::remove_from_index(&mut writer, doc_id)?;
        };

        // Remove from indexed_doc table
        if let Some(model) = indexed_document::Entity::find()
            .filter(indexed_document::Column::DocId.eq(doc_id))
            .one(&state.db)
            .await?
        {
            let _ = model.delete(&state.db).await;
        }

        Ok(())
    }

    /// Deletes multiple ids from the searcher at one time. The caller can decide if the
    /// documents should also be removed from the database by setting the remove_documents
    /// flag.
    pub async fn delete_many_by_id(
        state: &AppState,
        doc_ids: &[String],
        remove_documents: bool,
    ) -> anyhow::Result<()> {
        // Remove from search index, immediately.
        if let Ok(mut writer) = state.index.writer.lock() {
            Searcher::remove_many_from_index(&mut writer, doc_ids)?;
        };

        if remove_documents {
            // Remove from indexed_doc table
            let doc_refs: Vec<&str> = doc_ids.iter().map(AsRef::as_ref).collect();
            let docs = indexed_document::Entity::find()
                .filter(indexed_document::Column::DocId.is_in(doc_refs.clone()))
                .all(&state.db)
                .await?;

            let dbids: Vec<i64> = docs.iter().map(|x| x.id).collect();
            // Remove tags
            document_tag::Entity::delete_many()
                .filter(document_tag::Column::IndexedDocumentId.is_in(dbids))
                .exec(&state.db)
                .await?;

            indexed_document::Entity::delete_many()
                .filter(indexed_document::Column::DocId.is_in(doc_refs))
                .exec(&state.db)
                .await?;
        }
        Ok(())
    }

    pub async fn delete_by_url(state: &AppState, url: &str) -> anyhow::Result<()> {
        if let Some(model) = indexed_document::Entity::find()
            .filter(indexed_document::Column::Url.eq(url))
            .one(&state.db)
            .await?
        {
            Self::delete_by_id(state, &model.doc_id).await?;
        }

        Ok(())
    }

    /// Remove document w/ `doc_id` from the search index but will still have a
    /// reference in the database.
    pub fn remove_from_index(writer: &mut IndexWriter, doc_id: &str) -> anyhow::Result<()> {
        let fields = DocFields::as_fields();
        writer.delete_term(Term::from_field_text(fields.id, doc_id));
        Ok(())
    }

    /// Removes multiple documents from the index
    pub fn remove_many_from_index(
        writer: &mut IndexWriter,
        doc_ids: &[String],
    ) -> anyhow::Result<()> {
        let fields = DocFields::as_fields();
        for doc_id in doc_ids {
            writer.delete_term(Term::from_field_text(fields.id, doc_id));
        }

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
    pub fn with_index(index_path: &IndexPath) -> anyhow::Result<Self> {
        let schema = DocFields::as_schema();
        let index = match index_path {
            IndexPath::LocalPath(path) => {
                let dir = MmapDirectory::open(path)?;
                Index::open_or_create(dir, schema)?
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

        Ok(Searcher {
            index,
            reader,
            writer: Arc::new(Mutex::new(writer)),
        })
    }

    pub fn upsert_document(
        writer: &mut IndexWriter,
        doc_update: DocumentUpdate,
    ) -> tantivy::Result<String> {
        let fields = DocFields::as_fields();

        let doc_id = doc_update
            .doc_id
            .map_or_else(|| Uuid::new_v4().as_hyphenated().to_string(), |s| s);

        let mut doc = Document::default();
        doc.add_text(fields.content, doc_update.content);
        doc.add_text(fields.description, doc_update.description);
        doc.add_text(fields.domain, doc_update.domain);
        doc.add_text(fields.id, &doc_id);
        doc.add_text(fields.title, doc_update.title);
        doc.add_text(fields.url, doc_update.url);
        if let Some(tag) = doc_update.tags {
            for t in tag {
                doc.add_u64(fields.tags, *t);
            }
        }
        writer.add_document(doc)?;

        Ok(doc_id)
    }

    pub async fn search_with_lens(
        _db: DatabaseConnection,
        applied_lenses: &Vec<u64>,
        searcher: &Searcher,
        query_string: &str,
    ) -> Vec<SearchResult> {
        let start_timer = Instant::now();

        let index = &searcher.index;
        let reader = &searcher.reader;
        let fields = DocFields::as_fields();
        let searcher = reader.searcher();
        let tokenizers = index.tokenizers().clone();
        let query = build_query(
            index.schema(),
            tokenizers,
            fields,
            query_string,
            applied_lenses,
        );

        let collector = TopDocs::with_limit(5);

        let top_docs = searcher
            .search(&query, &collector)
            .expect("Unable to execute query");

        log::debug!(
            "query `{}` returned {} results from {} docs in {} ms",
            query_string,
            top_docs.len(),
            searcher.num_docs(),
            Instant::now().duration_since(start_timer).as_millis()
        );

        top_docs
            .into_iter()
            // Filter out negative scores
            .filter(|(score, _)| *score > 0.0)
            .collect()
    }
}

#[cfg(test)]
mod test {
    use crate::search::{DocumentUpdate, IndexPath, Searcher};
    use entities::models::create_connection;
    use shared::config::{Config, LensConfig};

    fn _build_test_index(searcher: &mut Searcher) {
        let writer = &mut searcher.writer.lock().unwrap();
        Searcher::upsert_document(
            writer,
            DocumentUpdate {
                doc_id: None,
                title: "Of Mice and Men",
                description: "Of Mice and Men passage",
                domain: "example.com",
                url: "https://example.com/mice_and_men",
                content:
                    "A few miles south of Soledad, the Salinas River drops in close to the hillside
            bank and runs deep and green. The water is warm too, for it has slipped twinkling
            over the yellow sands in the sunlight before reaching the narrow pool. On one
            side of the river the golden foothill slopes curve up to the strong and rocky
            Gabilan Mountains, but on the valley side the water is lined with trees—willows
            fresh and green with every spring, carrying in their lower leaf junctures the
            debris of the winter’s flooding; and sycamores with mottled, white, recumbent
            limbs and branches that arch over the pool",
                tags: &Some(vec![1 as u64]),
            },
        )
        .expect("Unable to add doc");

        Searcher::upsert_document(
            writer,
            DocumentUpdate {
                doc_id: None,
                title: "Of Mice and Men",
                description: "Of Mice and Men passage",
                domain: "en.wikipedia.org",
                url: "https://en.wikipedia.org/mice_and_men",
                content:
                    "A few miles south of Soledad, the Salinas River drops in close to the hillside
            bank and runs deep and green. The water is warm too, for it has slipped twinkling
            over the yellow sands in the sunlight before reaching the narrow pool. On one
            side of the river the golden foothill slopes curve up to the strong and rocky
            Gabilan Mountains, but on the valley side the water is lined with trees—willows
            fresh and green with every spring, carrying in their lower leaf junctures the
            debris of the winter’s flooding; and sycamores with mottled, white, recumbent
            limbs and branches that arch over the pool",
                tags: &Some(vec![2 as u64]),
            },
        )
        .expect("Unable to add doc");

        Searcher::upsert_document(
            writer,
            DocumentUpdate {
                doc_id: None,
                title: "Of Cheese and Crackers",
                description: "Of Cheese and Crackers Passage",
                domain: "en.wikipedia.org",
                url: "https://en.wikipedia.org/cheese_and_crackers",
                content: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Nulla
            tellus tortor, varius sit amet fermentum a, finibus porttitor erat. Proin
            suscipit, dui ac posuere vulputate, justo est faucibus est, a bibendum
            nulla nulla sed elit. Vivamus et libero a tortor ultricies feugiat in vel
            eros. Donec rhoncus mauris libero, et imperdiet neque sagittis sed. Nulla
            ac volutpat massa. Vivamus sed imperdiet est, id pretium ex. Praesent suscipit
            mattis ipsum, a lacinia nunc semper vitae.",
                tags: &Some(vec![2 as u64]),
            },
        )
        .expect("Unable to add doc");

        Searcher::upsert_document(
            writer,
            DocumentUpdate {
            doc_id: None,
            title:"Frankenstein: The Modern Prometheus",
            description: "A passage from Frankenstein",
            domain:"monster.com",
            url:"https://example.com/frankenstein",
            content:"You will rejoice to hear that no disaster has accompanied the commencement of an
             enterprise which you have regarded with such evil forebodings.  I arrived here
             yesterday, and my first task is to assure my dear sister of my welfare and
             increasing confidence in the success of my undertaking.",
             tags: &Some(vec![1 as u64]),}
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
        let _lens = LensConfig {
            name: "wiki".to_string(),
            domains: vec!["en.wikipedia.org".to_string()],
            urls: Vec::new(),
            ..Default::default()
        };

        let mut searcher = Searcher::with_index(&IndexPath::Memory).expect("Unable to open index");
        _build_test_index(&mut searcher);

        let query = "salinas";
        let results = Searcher::search_with_lens(db, &vec![2 as u64], &searcher, query).await;

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    pub async fn test_url_lens_search() {
        let db = create_connection(&Config::default(), true).await.unwrap();

        let _lens = LensConfig {
            name: "wiki".to_string(),
            domains: Vec::new(),
            urls: vec!["https://en.wikipedia.org/mice".to_string()],
            ..Default::default()
        };

        let mut searcher = Searcher::with_index(&IndexPath::Memory).expect("Unable to open index");

        _build_test_index(&mut searcher);
        let query = "salinas";
        let results = Searcher::search_with_lens(db, &vec![2 as u64], &searcher, query).await;

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    pub async fn test_singular_url_lens_search() {
        let db = create_connection(&Config::default(), true).await.unwrap();

        let _lens = LensConfig {
            name: "wiki".to_string(),
            domains: Vec::new(),
            urls: vec!["https://en.wikipedia.org/mice$".to_string()],
            ..Default::default()
        };

        let mut searcher = Searcher::with_index(&IndexPath::Memory).expect("Unable to open index");
        _build_test_index(&mut searcher);

        let query = "salinasd";
        let results = Searcher::search_with_lens(db, &vec![2 as u64], &searcher, query).await;
        assert_eq!(results.len(), 0);
    }
}
