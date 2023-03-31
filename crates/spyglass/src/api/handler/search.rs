use entities::models::tag::TagType;
use entities::models::{indexed_document, lens, tag};
use entities::schema::{DocFields, SearchDocument};
use entities::sea_orm::{
    self, prelude::*, sea_query::Expr, FromQueryResult, JoinType, QueryOrder, QuerySelect,
};
use jsonrpsee::core::Error;
use libspyglass::search::{document_to_struct, QueryStats, Searcher};
use libspyglass::state::AppState;
use libspyglass::task::{CleanupTask, ManagerCommand};
use shared::metrics;
use shared::request::{AskClippyRequest, LLMResponsePayload, SearchLensesParam, SearchParam};
use shared::response::{LensResult, SearchLensesResp, SearchMeta, SearchResult, SearchResults};
use spyglass_clippy::{unleash_clippy, TokenResult};
use spyglass_rpc::{RpcEvent, RpcEventType};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;
use tokio::sync::mpsc;
use tracing::instrument;

/// Max number of tokens we'll look at for matches before stopping.
const MAX_HIGHLIGHT_SCAN: usize = 10_000;
/// Max number of matches we need to generate a decent preview.
const MAX_HIGHLIGHT_MATCHES: usize = 5;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct WordRange {
    start: usize,
    end: usize,
    matches: Vec<usize>,
}

impl WordRange {
    pub fn new(start: usize, end: usize, match_idx: usize) -> Self {
        Self {
            start,
            end,
            matches: vec![match_idx],
        }
    }

    pub fn overlaps(&self, other: &WordRange) -> bool {
        self.start <= other.start && other.start <= self.end
            || self.start <= other.end && other.end <= self.end
    }

    pub fn merge(&mut self, other: &WordRange) {
        self.start = self.start.min(other.start);
        self.end = self.end.max(other.end);
        self.matches.extend(other.matches.iter());
    }
}

/// Creates a short preview from content based on the search query terms by
/// finding matches for words and creating a window around each match, joining
/// together overlaps & returning the final string.
fn generate_highlight_preview(index: &Searcher, query: &str, content: &str) -> String {
    let fields = DocFields::as_fields();
    let tokenizer = index
        .index
        .tokenizer_for_field(fields.content)
        .expect("Unable to get tokenizer for content field");

    // tokenize search query
    let mut terms = HashSet::new();
    let mut tokens = tokenizer.token_stream(query);
    while let Some(t) = tokens.next() {
        terms.insert(t.text.clone());
    }

    let tokens = content
        .split_whitespace()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let mut matched_indices = Vec::new();
    let mut num_tokens_scanned = 0;
    for (idx, w) in content.split_whitespace().enumerate() {
        num_tokens_scanned += 1;

        let normalized = tokenizer
            .token_stream(w)
            .next()
            .map(|t| t.text.clone())
            .unwrap_or_else(|| w.to_string());
        if terms.contains(&normalized) {
            matched_indices.push(idx);
        }

        if matched_indices.len() > MAX_HIGHLIGHT_MATCHES {
            break;
        }

        if num_tokens_scanned > MAX_HIGHLIGHT_SCAN {
            break;
        }
    }

    // Create word ranges from the indices
    let mut ranges: Vec<WordRange> = Vec::new();
    for idx in matched_indices {
        let start = (idx as i32 - 5).max(0) as usize;
        let end = (idx + 5).min(tokens.len() - 1);
        let new_range = WordRange::new(start, end, idx);

        if let Some(last) = ranges.last_mut() {
            if last.overlaps(&new_range) {
                last.merge(&new_range);
                continue;
            }
        }

        ranges.push(new_range);
    }

    // Create preview from word ranges
    let mut desc: Vec<String> = Vec::new();
    let mut num_windows = 0;
    for range in ranges {
        let mut slice = tokens[range.start..=range.end].to_vec();
        if !slice.is_empty() {
            for idx in range.matches {
                let slice_idx = idx - range.start;
                slice[slice_idx] = format!("<mark>{}</mark>", &slice[slice_idx]);
            }
            desc.extend(slice);
            desc.push("...".to_string());
            num_windows += 1;

            if num_windows > 3 {
                break;
            }
        }
    }

    format!("<span>{}</span>", desc.join(" "))
}

/// Ask clippy about a set of documents
#[instrument(skip(state))]
pub async fn ask_clippy(state: AppState, query: AskClippyRequest) -> Result<(), Error> {
    // Assumes a valid model has been downloaded and ready to go
    #[cfg(debug_assertions)]
    let model_path: PathBuf = "assets/models/alpaca-native.7b.bin".into();
    #[cfg(not(debug_assertions))]
    let model_path: PathBuf = state.config.model_dir().join("alpaca-native.7b.bin");

    log::debug!("ask_clippy: {:?}", query);
    let mut request = state.llm_request.lock().await;
    if request.is_none() {
        let s2 = state.clone();
        let handle = tokio::spawn(async move {
            let (tx, mut rx) = mpsc::unbounded_channel();
            // Spawn a task to send tokens to frontend
            tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    let mut finished = false;
                    let payload = match msg {
                        TokenResult::LoadingModel => {
                            serde_json::to_string(&LLMResponsePayload::LoadingModel)
                                .expect("Unable to serialize LLMResponse payload")
                        }
                        TokenResult::LoadingPrompt => {
                            serde_json::to_string(&LLMResponsePayload::LoadingPrompt)
                                .expect("Unable to serialize LLMResponse payload")
                        }
                        TokenResult::Token(c) => {
                            serde_json::to_string(&LLMResponsePayload::Token(c))
                                .expect("Unable to serialize LLMResponse payload")
                        }
                        TokenResult::Error(msg) => {
                            log::warn!("Received an error: {}", msg);
                            finished = true;
                            serde_json::to_string(&LLMResponsePayload::Error(msg))
                                .expect("Unable to serialize LLMResponse payload")
                        }
                        TokenResult::EndOfText => {
                            finished = true;
                            serde_json::to_string(&LLMResponsePayload::Finished)
                                .expect("Unable to serialize LLMResponse payload")
                        }
                    };

                    s2.publish_event(&RpcEvent {
                        event_type: RpcEventType::LLMResponse,
                        payload,
                    })
                    .await;

                    if finished {
                        let mut req = s2.llm_request.lock().await;
                        *req = None;
                        break;
                    }
                }
            });

            // Spawn the clippy LLM
            if let Err(err) = unleash_clippy(model_path, tx, &query.question, None, false) {
                log::warn!("Unable to complete clippy: {}", err);
            }
        });

        *request = Some(handle);
    } else {
        log::warn!("LLM request already underway");
    }

    Ok(())
}

/// Search the user's indexed documents
#[instrument(skip(state))]
pub async fn search_docs(state: AppState, search_req: SearchParam) -> Result<SearchResults, Error> {
    state
        .metrics
        .track(metrics::Event::Search {
            filters: search_req.lenses.clone(),
        })
        .await;

    let start = SystemTime::now();
    let index = &state.index;
    let searcher = index.reader.searcher();

    let tags = tag::Entity::find()
        .filter(tag::Column::Label.eq(tag::TagType::Lens.to_string()))
        .filter(tag::Column::Value.is_in(search_req.lenses))
        .all(&state.db)
        .await
        .unwrap_or_default();
    let tag_ids = tags
        .iter()
        .map(|model| model.id as u64)
        .collect::<Vec<u64>>();

    let mut stats = QueryStats::new();
    let docs = Searcher::search_with_lens(
        state.db.clone(),
        &tag_ids,
        index,
        &search_req.query,
        &mut stats,
    )
    .await;

    let mut results: Vec<SearchResult> = Vec::new();
    let mut missing: Vec<(String, String)> = Vec::new();

    let query = search_req.query.clone();
    for (score, doc_addr) in docs {
        if let Ok(Ok(doc)) = searcher.doc(doc_addr).map(|doc| document_to_struct(&doc)) {
            log::debug!("Got id with url {} {}", doc.doc_id, doc.url);
            let indexed = indexed_document::Entity::find()
                .filter(indexed_document::Column::DocId.eq(doc.doc_id.clone()))
                .one(&state.db)
                .await;

            let crawl_uri = doc.url;
            match indexed {
                Ok(Some(indexed)) => {
                    let tags = indexed
                        .find_related(tag::Entity)
                        .all(&state.db)
                        .await
                        .unwrap_or_default()
                        .iter()
                        .map(|tag| (tag.label.to_string(), tag.value.clone()))
                        .collect::<Vec<(String, String)>>();

                    let description =
                        generate_highlight_preview(&state.index, &query, &doc.content);
                    let result = SearchResult {
                        doc_id: doc.doc_id.clone(),
                        domain: doc.domain,
                        title: doc.title,
                        crawl_uri: crawl_uri.clone(),
                        description,
                        url: indexed.open_url.unwrap_or(crawl_uri),
                        tags,
                        score,
                    };

                    results.push(result);
                }
                _ => {
                    missing.push((doc.doc_id.to_owned(), crawl_uri.to_owned()));
                }
            }
        }
    }

    let wall_time_ms = SystemTime::now()
        .duration_since(start)
        .map_or_else(|_| 0, |duration| duration.as_millis() as u64);

    let num_docs = searcher.num_docs();
    let meta = SearchMeta {
        query: search_req.query.clone(),
        num_docs: num_docs as u32,
        wall_time_ms: wall_time_ms as u32,
    };

    let domains: HashSet<String> = HashSet::from_iter(results.iter().map(|r| r.domain.clone()));
    state
        .metrics
        .track(metrics::Event::SearchResult {
            num_results: results.len(),
            num_docs,
            term_count: stats.term_count,
            domains: domains.iter().cloned().collect(),
            wall_time_ms,
        })
        .await;

    // Send cleanup task for any missing docs
    if !missing.is_empty() {
        let mut cmd_tx = state.manager_cmd_tx.lock().await;
        match &mut *cmd_tx {
            Some(cmd_tx) => {
                let _ = cmd_tx.send(ManagerCommand::CleanupDatabase(CleanupTask {
                    missing_docs: missing,
                }));
            }
            None => {}
        }
    }

    Ok(SearchResults { results, meta })
}

#[derive(FromQueryResult)]
struct LensSearch {
    author: Option<String>,
    name: String,
    description: Option<String>,
}

/// Search the user's installed lenses
#[instrument(skip(state))]
pub async fn search_lenses(
    state: AppState,
    param: SearchLensesParam,
) -> Result<SearchLensesResp, Error> {
    let mut results = Vec::new();
    let query_result = tag::Entity::find()
        .column_as(tag::Column::Value, "name")
        .column_as(lens::Column::Author, "author")
        .column_as(lens::Column::Description, "description")
        .filter(tag::Column::Label.eq(TagType::Lens.to_string()))
        .filter(tag::Column::Value.like(&format!("%{}%", &param.query)))
        // Pull in lens metadata
        .join_rev(
            JoinType::LeftJoin,
            lens::Entity::belongs_to(tag::Entity)
                .from(lens::Column::Name)
                .to(tag::Column::Value)
                .into(),
        )
        // Order by trigger name, case insensitve
        .order_by_asc(Expr::cust("lower(value)"))
        .into_model::<LensSearch>()
        .all(&state.db)
        .await
        .unwrap_or_default();

    for lens in query_result {
        let label = lens.name.clone();
        results.push(LensResult {
            author: lens.author.unwrap_or("spyglass-search".into()),
            name: label.clone(),
            label,
            description: lens.description.unwrap_or_default(),
            ..Default::default()
        });
    }

    // brute force add clippy here
    if param.query.is_empty() || "ask".starts_with(&param.query) {
        results.push(LensResult {
            author: "spyglass-search".into(),
            name: "Ask".into(),
            label: "ask".into(),
            description: "Ask questions about your data".into(),
            ..Default::default()
        });
    }

    Ok(SearchLensesResp { results })
}

#[cfg(test)]
mod test {
    use crate::api::handler::search::generate_highlight_preview;
    use libspyglass::search::{IndexPath, Searcher};

    #[test]
    fn test_find_highlights() {
        let searcher = Searcher::with_index(&IndexPath::Memory).expect("Unable to open index");
        let blurb = r#"Rust rust is a multi-paradigm, high-level, general-purpose programming"#;
        let desc = generate_highlight_preview(&searcher, "rust programming", &blurb);
        assert_eq!(desc, "<span><mark>Rust</mark> <mark>rust</mark> is a multi-paradigm, high-level, general-purpose <mark>programming</mark> ...</span>");
    }
}
