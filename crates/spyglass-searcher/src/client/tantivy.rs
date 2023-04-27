use std::collections::HashSet;
use std::fmt::{Debug, Error, Formatter};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, TermQuery};
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy};
use uuid::Uuid;

use crate::{SearchError, SearcherResult, document_to_struct, QueryBoost, RetrievedDocument, Score, DocumentUpdate, IndexPath};
use crate::schema::{self, DocFields, SearchDocument};
use crate::query::{self, QueryStats};
use crate::query::{build_document_query, build_query, QueryBoosts};

/// Tantivy searcher client
#[derive(Clone)]
pub struct Searcher {
    pub index: Index,
    pub reader: IndexReader,
    pub writer: Option<Arc<Mutex<IndexWriter>>>,
}

impl Debug for Searcher {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.debug_struct("Searcher")
            .field("index", &self.index)
            .finish()
    }
}

impl Searcher {
    pub fn is_readonly(&self) -> bool {
        self.writer.is_none()
    }

    pub fn lock_writer(&self) -> SearcherResult<MutexGuard<IndexWriter>> {
        if let Some(index) = &self.writer {
            match index.lock() {
                Ok(lock) => Ok(lock),
                Err(_) => Err(SearchError::WriterLocked),
            }
        } else {
            Err(SearchError::ReadOnly)
        }
    }

    pub async fn save(&self) -> SearcherResult<()> {
        let mut writer = self.lock_writer()?;
        writer.commit()?;
        Ok(())
    }

    /// Deletes a single entry from the database & index
    pub async fn delete_by_id(&self, doc_id: &str) -> SearcherResult<()> {
        self.delete_many_by_id(&[doc_id.into()]).await?;
        Ok(())
    }

    /// Deletes multiple ids from the searcher at one time. The caller can decide if the
    /// documents should also be removed from the database by setting the remove_documents
    /// flag.
    pub async fn delete_many_by_id(&self, doc_ids: &[String]) -> SearcherResult<()> {
        {
            let writer = self.lock_writer()?;
            let fields = DocFields::as_fields();
            for doc_id in doc_ids {
                writer.delete_term(Term::from_field_text(fields.id, doc_id));
            }
        }

        self.save().await?;
        Ok(())
    }

    /// Get document with `doc_id` from index.
    pub fn get_by_id(&self, doc_id: &str) -> Option<Document> {
        let fields = DocFields::as_fields();
        let searcher = self.reader.searcher();

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
    pub fn with_index(index_path: &IndexPath, readonly: bool) -> SearcherResult<Self> {
        let index = match index_path {
            IndexPath::LocalPath(path) => schema::initialize_index(path)?,
            IndexPath::Memory => schema::initialize_in_memory_index(),
        };

        // Should only be one writer at a time. This single IndexWriter is already
        // multithreaded.
        let writer = if readonly {
            None
        } else {
            Some(Arc::new(Mutex::new(
                index
                    .writer(50_000_000)
                    .expect("Unable to create index_writer"),
            )))
        };

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
            writer,
        })
    }

    pub fn upsert_document(&self, doc_update: DocumentUpdate) -> SearcherResult<String> {
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
        for t in doc_update.tags {
            doc.add_u64(fields.tags, *t as u64);
        }

        let writer = self.lock_writer()?;
        writer.add_document(doc)?;

        Ok(doc_id)
    }

    /// Helper method to execute a search based on the provided document query
    pub async fn search_by_query(
        &self,
        urls: Option<Vec<String>>,
        ids: Option<Vec<String>>,
        has_tags: &[u64],
        exclude_tags: &[u64],
    ) -> Vec<(Score, RetrievedDocument)> {
        let urls = urls.unwrap_or_default();
        let ids = ids.unwrap_or_default();

        let fields = DocFields::as_fields();
        let query = build_document_query(fields, &urls, &ids, has_tags, exclude_tags);

        let collector = tantivy::collector::DocSetCollector;

        let reader = &self.reader;
        let index_search = reader.searcher();

        let docs = index_search
            .search(&query, &collector)
            .expect("Unable to execute query");

        docs.into_iter()
            .map(|addr| (1.0, addr))
            .flat_map(|(score, addr)| {
                if let Ok(Some(doc)) = index_search.doc(addr).map(|x| document_to_struct(&x)) {
                    Some((score, doc))
                } else {
                    None
                }
            })
            .collect()
    }

    pub async fn search_with_lens(
        &self,
        applied_lenses: &Vec<u64>,
        query_string: &str,
        favorite_id: Option<u64>,
        boosts: &[QueryBoost],
        stats: &mut QueryStats,
        num_results: usize,
    ) -> Vec<(Score, RetrievedDocument)> {
        let start_timer = Instant::now();
        let index = &self.index;
        let reader = &self.reader;
        let fields = DocFields::as_fields();
        let searcher = reader.searcher();
        let tokenizers = index.tokenizers().clone();

        let mut tag_boosts = HashSet::new();
        let mut docid_boosts = Vec::new();
        let mut url_boosts = Vec::new();
        for boost in boosts {
            match boost {
                QueryBoost::DocId(doc_id) => docid_boosts.push(doc_id.clone()),
                QueryBoost::Url(url) => url_boosts.push(url.clone()),
                QueryBoost::Tag(tag_id) => {
                    tag_boosts.insert(*tag_id);
                }
            }
        }

        let boosts = QueryBoosts {
            tags: tag_boosts.into_iter().collect(),
            favorite: favorite_id,
            urls: url_boosts,
            doc_ids: docid_boosts,
        };

        let query = build_query(
            index.schema(),
            tokenizers,
            fields,
            query_string,
            applied_lenses,
            stats,
            &boosts,
        );

        let collector = TopDocs::with_limit(num_results);

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

        let doc_reader = self.reader.searcher();
        top_docs
            .into_iter()
            // Filter out negative scores
            .filter(|(score, _)| *score > 0.0)
            .flat_map(|(score, addr)| {
                if let Ok(Some(doc)) = doc_reader.doc(addr).map(|x| document_to_struct(&x)) {
                    Some((score, doc))
                } else {
                    None
                }
            })
            .collect()
    }

    pub async fn explain_search_with_lens(
        &self,
        doc: RetrievedDocument,
        applied_lenses: &Vec<u64>,
        query_string: &str,
        favorite_id: Option<u64>,
        boosts: &[QueryBoost],
        stats: &mut QueryStats,
    ) -> Option<f32> {
        let mut tag_boosts = HashSet::new();
        let mut docid_boosts = Vec::new();
        let mut url_boosts = Vec::new();
        for boost in boosts {
            match boost {
                QueryBoost::DocId(doc_id) => docid_boosts.push(doc_id.clone()),
                QueryBoost::Url(url) => url_boosts.push(url.clone()),
                QueryBoost::Tag(tag_id) => {
                    tag_boosts.insert(*tag_id);
                }
            }
        }

        let index = &self.index;
        let reader = &self.reader;
        let fields = DocFields::as_fields();

        let tantivy_searcher = reader.searcher();
        let tokenizers = index.tokenizers().clone();
        let boosts = QueryBoosts {
            tags: tag_boosts.into_iter().collect(),
            favorite: favorite_id,
            urls: url_boosts,
            doc_ids: docid_boosts,
        };

        let query = build_query(
            index.schema(),
            tokenizers.clone(),
            fields.clone(),
            query_string,
            applied_lenses,
            stats,
            &boosts,
        );

        let mut combined: Vec<(Occur, Box<dyn Query>)> = vec![(Occur::Should, Box::new(query))];
        combined.push((
            Occur::Must,
            Box::new(TermQuery::new(
                Term::from_field_text(fields.id, &doc.doc_id),
                // Needs WithFreqs otherwise scoring is wonky.
                IndexRecordOption::WithFreqs,
            )),
        ));

        let content_terms =
            query::terms_for_field(&index.schema(), &tokenizers, query_string, fields.content);
        log::info!("Content Tokens {:?}", content_terms);

        let final_query = BooleanQuery::new(combined);
        let collector = tantivy::collector::TopDocs::with_limit(1);

        let docs = tantivy_searcher
            .search(&final_query, &collector)
            .expect("Unable to execute query");
        for (score, addr) in docs {
            if let Ok(Some(result)) = tantivy_searcher.doc(addr).map(|x| document_to_struct(&x)) {
                if result.doc_id == doc.doc_id {
                    for t in content_terms {
                        let info = tantivy_searcher
                            .segment_reader(addr.segment_ord)
                            .inverted_index(fields.content)
                            .unwrap()
                            .get_term_info(&t.1);
                        log::info!("Term {:?} Info {:?} ", t, info);
                    }

                    return Some(score);
                }
            }
        }
        None
    }
}