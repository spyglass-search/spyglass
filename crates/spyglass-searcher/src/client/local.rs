use std::fmt::{Debug, Error, Formatter};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use tantivy::collector::TopDocs;
use tantivy::query::TermQuery;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy};
use uuid::Uuid;

use crate::query::{build_document_query, build_query, terms_for_field, QueryOptions};
use crate::schema::{self, DocFields, SearchDocument};
use crate::{
    document_to_struct, Boost, DocumentUpdate, IndexBackend, QueryBoost, RetrievedDocument, Score,
    SearchError, SearchQueryResult, SearchTrait, SearcherResult, WriteTrait,
};

const SPYGLASS_NS: Uuid = uuid::uuid!("5fdfe40a-de2c-11ed-bfa7-00155deae876");

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

#[async_trait::async_trait]
impl WriteTrait for Searcher {
    async fn delete_many_by_id(&self, doc_ids: &[String]) -> SearcherResult<usize> {
        {
            let writer = self.lock_writer()?;
            let fields = DocFields::as_fields();
            for doc_id in doc_ids {
                writer.delete_term(Term::from_field_text(fields.id, doc_id));
            }
        }

        self.save().await?;
        Ok(doc_ids.len())
    }

    async fn upsert_many(&self, updates: &[DocumentUpdate]) -> SearcherResult<Vec<String>> {
        let mut upserted = Vec::new();
        for doc_update in updates {
            let fields = DocFields::as_fields();

            let doc_id = doc_update.doc_id.clone().map_or_else(
                || {
                    Uuid::new_v5(&SPYGLASS_NS, doc_update.url.as_bytes())
                        .as_hyphenated()
                        .to_string()
                },
                |s| s,
            );

            let mut doc = Document::default();
            doc.add_text(fields.content, doc_update.content);
            doc.add_text(fields.domain, doc_update.domain);
            doc.add_text(fields.id, &doc_id);
            doc.add_text(fields.title, doc_update.title);
            doc.add_text(fields.url, doc_update.url);
            for t in doc_update.tags {
                doc.add_u64(fields.tags, *t as u64);
            }

            let writer = self.lock_writer()?;
            writer.add_document(doc)?;

            upserted.push(doc_id.clone());
        }

        Ok(upserted)
    }
}

#[async_trait::async_trait]
impl SearchTrait for Searcher {
    /// Get a single document by id
    async fn get(&self, doc_id: &str) -> Option<RetrievedDocument> {
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
                return document_to_struct(&doc);
            }
        }

        None
    }

    /// Runs a search against the index
    async fn search(
        &self,
        query_string: &str,
        filters: &[QueryBoost],
        boosts: &[QueryBoost],
        num_results: usize,
    ) -> SearchQueryResult {
        let start_timer = Instant::now();

        let index = &self.index;
        let reader = &self.reader;
        let searcher = reader.searcher();

        let (term_counts, query) = build_query(
            index,
            query_string,
            filters,
            boosts,
            QueryOptions::default(),
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
        let docs = top_docs
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
            .collect();

        SearchQueryResult {
            wall_time_ms: Instant::now().duration_since(start_timer).as_millis(),
            num_docs: searcher.num_docs(),
            term_counts,
            documents: docs,
        }
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

    /// Constructs a new Searcher object w/ the index @ `index_path`
    pub fn with_index(
        index_path: &IndexBackend,
        schema: Schema,
        readonly: bool,
    ) -> SearcherResult<Self> {
        let index = match index_path {
            IndexBackend::LocalPath(path) => schema::initialize_index(schema, path)?,
            IndexBackend::Memory => schema::initialize_in_memory_index(schema),
            IndexBackend::Http(_) => unimplemented!(""),
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

    pub async fn explain_search_with_lens(
        &self,
        doc_id: String,
        query_string: &str,
        boosts: &[QueryBoost],
    ) -> Option<f32> {
        let index = &self.index;
        let reader = &self.reader;
        let fields = DocFields::as_fields();

        let tantivy_searcher = reader.searcher();
        let filters = vec![QueryBoost::new(Boost::DocId(doc_id.clone()))];
        let (_, final_query) = build_query(
            &self.index,
            query_string,
            &filters,
            boosts,
            QueryOptions::default(),
        );

        let tokenizers = index.tokenizers();
        let content_terms =
            terms_for_field(&index.schema(), tokenizers, query_string, fields.content);
        log::info!("Content Tokens {:?}", content_terms);

        let collector = tantivy::collector::TopDocs::with_limit(1);
        let docs = tantivy_searcher
            .search(&final_query, &collector)
            .expect("Unable to execute query");

        for (score, addr) in docs {
            if let Ok(Some(result)) = tantivy_searcher.doc(addr).map(|x| document_to_struct(&x)) {
                if result.doc_id == doc_id {
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
