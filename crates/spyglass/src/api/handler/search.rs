use entities::models::tag::TagType;
use entities::models::{indexed_document, lens, tag};
use entities::sea_orm;
use entities::sea_orm::{prelude::*, sea_query::Expr, QueryOrder};
use entities::sea_orm::{FromQueryResult, JoinType, QuerySelect};
use jsonrpsee::core::Error;
use libspyglass::search::{document_to_struct, Searcher};
use libspyglass::state::AppState;
use libspyglass::task::{CleanupTask, ManagerCommand};
use shared::metrics;
use shared::request;
use shared::response::{LensResult, SearchLensesResp, SearchMeta, SearchResult, SearchResults};
use std::collections::HashSet;
use std::time::SystemTime;
use tracing::instrument;

/// Search the user's indexed documents
#[instrument(skip(state))]
pub async fn search_docs(
    state: AppState,
    search_req: request::SearchParam,
) -> Result<SearchResults, Error> {
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
        .filter(tag::Column::Label.eq(tag::TagType::Lens))
        .filter(tag::Column::Value.is_in(search_req.lenses))
        .all(&state.db)
        .await
        .unwrap_or_default();
    let tag_ids = tags
        .iter()
        .map(|model| model.id as u64)
        .collect::<Vec<u64>>();

    let docs =
        Searcher::search_with_lens(state.db.clone(), &tag_ids, index, &search_req.query).await;

    let mut results: Vec<SearchResult> = Vec::new();
    let mut missing: Vec<(String, String)> = Vec::new();

    let terms = search_req.query.split_whitespace().collect::<HashSet<_>>();
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
                        .map(|tag| (tag.label.as_ref().to_string(), tag.value.clone()))
                        .collect::<Vec<(String, String)>>();

                    let matched_indices = doc
                        .content
                        .split_whitespace()
                        .enumerate()
                        .filter(|(_, w)| terms.contains(w))
                        .map(|(idx, _)| idx)
                        .collect::<Vec<_>>();

                    dbg!(matched_indices);
                    let result = SearchResult {
                        doc_id: doc.doc_id.clone(),
                        domain: doc.domain,
                        title: doc.title,
                        crawl_uri: crawl_uri.clone(),
                        description: doc.description,
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

    let meta = SearchMeta {
        query: search_req.query,
        num_docs: searcher.num_docs() as u32,
        wall_time_ms: wall_time_ms as u32,
    };

    let domains: HashSet<String> = HashSet::from_iter(results.iter().map(|r| r.domain.clone()));
    state
        .metrics
        .track(metrics::Event::SearchResult {
            num_results: results.len(),
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
    param: request::SearchLensesParam,
) -> Result<SearchLensesResp, Error> {
    let mut results = Vec::new();
    let query_result = tag::Entity::find()
        .column_as(tag::Column::Value, "name")
        .column_as(lens::Column::Author, "author")
        .column_as(lens::Column::Description, "description")
        .filter(tag::Column::Label.eq(TagType::Lens))
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

    Ok(SearchLensesResp { results })
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    #[test]
    fn test_find_highlights() {
        let terms = HashSet::from(["rust".to_string(), "programming".to_string()]);

        let blurb = r#"Rust is a multi-paradigm, high-level, general-purpose programming language.
            Rust emphasizes performance, type safety, and concurrency. Rust enforces memory safety—that is,
            that all references point to valid memory—without requiring the use of a garbage collector or
            reference counting present in other memory-safe languages. To simultaneously enforce memory safety
            and prevent concurrent data races, Rust's "borrow checker" tracks the object lifetime of all
            references in a program during compilation. Rust is popular for systems programming but also offers
            high-level features including some functional programming constructs."#;

        let words = blurb
            .split_whitespace()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let matched_indices = words
            .iter()
            .enumerate()
            .filter(|(_, w)| terms.contains(&w.to_lowercase()))
            .map(|(idx, _)| idx)
            .collect::<Vec<_>>();

        // create a summary from the first 3 matches
        let mut desc: Vec<String> = Vec::new();
        let mut ranges = Vec::new();

        for idx in matched_indices {
            let start = (idx as i32 - 5).max(0) as usize;
            let end = (idx + 5).min(words.len() - 1);
            let range = Range { start, end };

            ranges.push(range);
            desc.extend(words[start..end].iter().map(|s| s.to_owned()));
        }

        dbg!(desc.join(" "));
    }
}
