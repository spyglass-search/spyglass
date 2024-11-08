use entities::models::tag::{check_query_for_tags, get_favorite_tag, TagType};
use entities::models::vec_documents::DocDistance;
use entities::models::{indexed_document, lens, tag, vec_documents};
use entities::sea_orm::{
    self, prelude::*, sea_query::Expr, FromQueryResult, JoinType, QueryOrder, QuerySelect,
};
use jsonrpsee::core::RpcResult;
use libspyglass::state::AppState;
use libspyglass::task::{CleanupTask, ManagerCommand};
use shared::metrics;
use shared::request;
use shared::response::{LensResult, SearchLensesResp, SearchMeta, SearchResult, SearchResults};
use spyglass_model_interface::embedding_api::EmbeddingContentType;
use spyglass_searcher::schema::{DocFields, SearchDocument};
use spyglass_searcher::{Boost, QueryBoost, SearchTrait};
use std::collections::HashSet;
use std::time::SystemTime;
use tracing::instrument;

/// Search the user's indexed documents
#[instrument(skip(state))]
pub async fn search_docs(
    state: AppState,
    search_req: request::SearchParam,
) -> RpcResult<SearchResults> {
    state
        .metrics
        .track(metrics::Event::Search {
            filters: search_req.lenses.clone(),
        })
        .await;

    let start = SystemTime::now();
    let index = &state.index;
    let searcher = index.reader.searcher();
    let query = search_req.query.clone();

    let lens_ids = tag::Entity::find()
        .filter(tag::Column::Label.eq(tag::TagType::Lens.to_string()))
        .filter(tag::Column::Value.is_in(search_req.lenses))
        .all(&state.db)
        .await
        .unwrap_or_default()
        .iter()
        .map(|model| model.id as u64)
        .collect::<Vec<u64>>();

    let mut boosts = Vec::new();
    for tag in check_query_for_tags(&state.db, &query).await {
        boosts.push(QueryBoost::new(Boost::Tag(tag)))
    }

    let mut filters = Vec::new();
    for lens in &lens_ids {
        filters.push(QueryBoost::new(Boost::Tag(*lens)));
    }

    if let Some(tag_id) = get_favorite_tag(&state.db).await {
        filters.push(QueryBoost::new(Boost::Favorite {
            id: tag_id,
            required: false,
        }));
    }

    if let Some(embedding_api) = state.embedding_api.as_ref() {
        match embedding_api.embed(&query, EmbeddingContentType::Query) {
            Ok(embedding) => {
                let mut distances =
                    vec_documents::get_document_distance(&state.db, &lens_ids, &embedding).await;

                if let Ok(distances) = distances.as_mut() {
                    let mut distances = distances
                        .iter()
                        .filter(|dist| dist.distance < 25.0)
                        .collect::<Vec<&DocDistance>>();
                    distances.sort_by(|a, b| a.distance.total_cmp(&b.distance));

                    let min_value = distances
                        .iter()
                        .map(|distance| distance.distance)
                        .reduce(f64::min);
                    let max_value = distances
                        .iter()
                        .map(|distance| distance.distance)
                        .reduce(f64::max);
                    if let (Some(min), Some(max)) = (min_value, max_value) {
                        for distance in distances {
                            let boost_normalized = (distance.distance - min) / (max - min) * 3.0;
                            let boost = 3.0 - boost_normalized;

                            boosts.push(QueryBoost::with_value(
                                Boost::DocId(distance.doc_id.clone()),
                                boost as f32,
                            ));
                        }
                    }
                }
            }
            Err(err) => {
                log::error!("Error embedding query {:?}", err);
            }
        }
    }

    let search_result = state.index.search(&query, &filters, &boosts, 5).await;
    log::debug!(
        "query {}: {} results from {} docs in {}ms",
        query,
        search_result.documents.len(),
        search_result.num_docs,
        search_result.wall_time_ms
    );

    let mut results: Vec<SearchResult> = Vec::new();
    let mut missing: Vec<(String, String)> = Vec::new();
    for (score, doc) in search_result.documents {
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

                let fields = DocFields::as_fields();
                let tokenizer = index
                    .index
                    .tokenizer_for_field(fields.content)
                    .expect("Unable to get tokenizer for content field");

                let description = spyglass_searcher::utils::generate_highlight_preview(
                    &tokenizer,
                    &query,
                    &doc.content,
                );

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
            term_count: search_result.term_counts as i32,
            domains: domains.iter().cloned().collect(),
            wall_time_ms,
        })
        .await;

    // Send cleanup task for any missing docs
    if !missing.is_empty() {
        let mut cmd_tx = state.manager_cmd_tx.lock().await;
        if let Some(cmd_tx) = &mut *cmd_tx {
            let _ = cmd_tx.send(ManagerCommand::CleanupDatabase(CleanupTask {
                missing_docs: missing,
            }));
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
) -> RpcResult<SearchLensesResp> {
    let mut results = Vec::new();
    let query_result = tag::Entity::find()
        .column_as(tag::Column::Value, "name")
        .column_as(lens::Column::Author, "author")
        .column_as(lens::Column::Description, "description")
        .filter(tag::Column::Label.eq(TagType::Lens.to_string()))
        .filter(tag::Column::Value.like(format!("%{}%", &param.query)))
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
