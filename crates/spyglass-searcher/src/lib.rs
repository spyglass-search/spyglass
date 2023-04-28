use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::path::PathBuf;
use tantivy::schema::*;
use thiserror::Error;
use url::Url;

pub mod client;
pub mod schema;
pub mod stop_word_filter;
use schema::{DocFields, DocumentUpdate, SearchDocument};

mod query;
pub mod similarity;
pub mod utils;

type Score = f32;

pub enum IndexBackend {
    // Elasticsearch compatible REST API (such as Quickwit for example)
    Http(Url),
    // Directory
    LocalPath(PathBuf),
    // In memory index for testing purposes.
    Memory,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct QueryBoost {
    /// What to boost
    field: Boost,
    /// The boost value (negative to lessen impact of something)
    value: f32,
}

impl QueryBoost {
    pub fn new(boost: Boost) -> Self {
        let value = &match boost {
            Boost::DocId(_) => 3.0,
            Boost::Favorite { .. } => 3.0,
            Boost::Tag(_) => 1.5,
            Boost::Url(_) => 3.0,
        };

        QueryBoost {
            field: boost,
            value: *value,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Boost {
    // If required is set to true, _only_ favorites will be searched.
    Favorite { id: u64, required: bool },
    Url(String),
    DocId(String),
    Tag(u64),
}

/// Contains stats & results for a search request
#[derive(Clone)]
pub struct SearchQueryResult {
    pub wall_time_ms: u128,
    pub num_docs: u64,
    pub term_counts: usize,
    pub documents: Vec<(Score, RetrievedDocument)>,
}

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum SearchError {
    #[error("Unable to perform action on index: {0}")]
    IndexError(#[from] tantivy::TantivyError),
    #[error("Index is in read only mode")]
    ReadOnly,
    #[error("Index writer is deadlocked")]
    WriterLocked,
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

/// Generic API for an index that can perform queries & get specific documents.
#[async_trait::async_trait]
pub trait SearchTrait {
    /// Get a single document by id
    async fn get(&self, doc_id: &str) -> Option<RetrievedDocument>;
    /// Runs a search against the index
    async fn search(
        &self,
        query: &str,
        filters: &[QueryBoost],
        boosts: &[QueryBoost],
        num_results: usize,
    ) -> SearchQueryResult;
}

#[async_trait::async_trait]
pub trait WriteTrait {
    /// Delete a single document.
    async fn delete(&self, doc_id: &str) -> SearcherResult<()> {
        self.delete_many_by_id(&[doc_id.to_owned()]).await?;
        Ok(())
    }
    /// Delete documents from the index by id, returning the number of docs deleted.
    async fn delete_many_by_id(&self, doc_ids: &[String]) -> SearcherResult<usize>;

    async fn upsert(&self, single_doc: &Document) -> SearcherResult<String> {
        let upserted = self.upsert_many(&[single_doc.clone()]).await?;
        Ok(upserted.get(0).expect("Expected a single doc").to_owned())
    }

    /// Insert/update documents in the index, returning the list of document ids
    async fn upsert_many(&self, updates: &[Document]) -> SearcherResult<Vec<String>>;
}

type SearcherResult<T> = Result<T, SearchError>;

#[derive(Clone, Serialize)]
pub struct RetrievedDocument {
    pub doc_id: String,
    pub domain: String,
    pub title: String,
    pub description: String,
    pub content: String,
    pub url: String,
    pub tags: Vec<u64>,
}

// Helper method used to get the string value from a field
fn field_to_string(doc: &Document, field: Field) -> String {
    doc.get_first(field)
        .map(|x| x.as_text().unwrap_or_default())
        .map(|x| x.to_string())
        .unwrap_or_default()
}

// Helper method used to get the u64 vector from a field.
fn field_to_u64vec(doc: &Document, field: Field) -> Vec<u64> {
    doc.get_all(field).filter_map(|val| val.as_u64()).collect()
}

/// Helper method used to convert the provided document to a struct
pub fn document_to_struct(doc: &Document) -> Option<RetrievedDocument> {
    let fields = DocFields::as_fields();
    let doc_id = field_to_string(doc, fields.id);
    if doc_id.is_empty() {
        return None;
    }

    let domain = field_to_string(doc, fields.domain);
    let title = field_to_string(doc, fields.title);
    let description = field_to_string(doc, fields.description);
    let url = field_to_string(doc, fields.url);
    let content = field_to_string(doc, fields.content);
    let tags = field_to_u64vec(doc, fields.tags);

    Some(RetrievedDocument {
        doc_id,
        domain,
        title,
        description,
        content,
        url,
        tags,
    })
}

#[cfg(test)]
mod test {
    use crate::client::Searcher;
    use crate::schema::{DocFields, SearchDocument};
    use crate::{Boost, DocumentUpdate, IndexBackend, QueryBoost, SearchTrait, WriteTrait};

    async fn _build_test_index(searcher: &mut Searcher) {
        searcher
            .upsert(&DocumentUpdate {
                doc_id: None,
                title: "Of Mice and Men",
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
                tags: &vec![1_i64],
                published_at: None,
                last_modified: None,
            }.to_document())
            .await
            .expect("Unable to add doc");

        searcher
            .upsert(&DocumentUpdate {
                doc_id: None,
                title: "Of Mice and Men",
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
                tags: &vec![2_i64],
                published_at: None,
                last_modified: None,
            }.to_document())
            .await
            .expect("Unable to add doc");

        searcher
            .upsert(
                &DocumentUpdate {
                    doc_id: None,
                    title: "Of Cheese and Crackers",
                    domain: "en.wikipedia.org",
                    url: "https://en.wikipedia.org/cheese_and_crackers",
                    content: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Nulla
            tellus tortor, varius sit amet fermentum a, finibus porttitor erat. Proin
            suscipit, dui ac posuere vulputate, justo est faucibus est, a bibendum
            nulla nulla sed elit. Vivamus et libero a tortor ultricies feugiat in vel
            eros. Donec rhoncus mauris libero, et imperdiet neque sagittis sed. Nulla
            ac volutpat massa. Vivamus sed imperdiet est, id pretium ex. Praesent suscipit
            mattis ipsum, a lacinia nunc semper vitae.",
                    tags: &vec![2_i64],
                    published_at: None,
                    last_modified: None,
                }
                .to_document(),
            )
            .await
            .expect("Unable to add doc");

        searcher.upsert(
            &DocumentUpdate {
            doc_id: None,
            title:"Frankenstein: The Modern Prometheus",
            domain:"monster.com",
            url:"https://example.com/frankenstein",
            content:"You will rejoice to hear that no disaster has accompanied the commencement of an
             enterprise which you have regarded with such evil forebodings.  I arrived here
             yesterday, and my first task is to assure my dear sister of my welfare and
             increasing confidence in the success of my undertaking.",
             tags: &vec![1_i64],
             published_at: None,
             last_modified: None
        }.to_document()).await
        .expect("Unable to add doc");

        let res = searcher.save().await;
        if let Err(err) = res {
            println!("{err:?}");
        }

        // add a small delay so that the documents can be properly committed
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }

    #[tokio::test]
    pub async fn test_basic_lense_search() {
        let mut searcher =
            Searcher::with_index(&IndexBackend::Memory, DocFields::as_schema(), false)
                .expect("Unable to open index");
        _build_test_index(&mut searcher).await;

        let query = "salinas";
        let filters = vec![QueryBoost::new(Boost::Tag(2_u64))];
        let results = searcher.search(query, &filters, &[], 5).await;
        assert_eq!(results.documents.len(), 1);
    }

    #[tokio::test]
    pub async fn test_url_lens_search() {
        let mut searcher =
            Searcher::with_index(&IndexBackend::Memory, DocFields::as_schema(), false)
                .expect("Unable to open index");
        _build_test_index(&mut searcher).await;

        let query = "salinas";
        let filters = vec![QueryBoost::new(Boost::Tag(2_u64))];
        let results = searcher.search(query, &filters, &[], 5).await;
        assert_eq!(results.documents.len(), 1);
    }

    #[tokio::test]
    pub async fn test_singular_url_lens_search() {
        let mut searcher =
            Searcher::with_index(&IndexBackend::Memory, DocFields::as_schema(), false)
                .expect("Unable to open index");
        _build_test_index(&mut searcher).await;

        let query = "salinasd";
        let filters = vec![QueryBoost::new(Boost::Tag(2_u64))];
        let results = searcher.search(query, &filters, &[], 5).await;
        assert_eq!(results.documents.len(), 0);
    }
}
