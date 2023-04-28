use tantivy::query::{BooleanQuery, BoostQuery, Occur, PhraseQuery, Query, TermQuery};
use tantivy::tokenizer::*;
use tantivy::Score;
use tantivy::{schema::*, Index};

use crate::schema::SearchDocument;
use crate::{Boost, QueryBoost};

use super::DocFields;

type QueryVec = Vec<(Occur, Box<dyn Query>)>;

fn _boosted_term(term: Term, boost: Score) -> Box<BoostQuery> {
    Box::new(BoostQuery::new(
        Box::new(TermQuery::new(
            term,
            // Needs WithFreqs otherwise scoring is wonky.
            IndexRecordOption::WithFreqs,
        )),
        boost,
    ))
}

fn _boosted_phrase(terms: Vec<(usize, Term)>, boost: Score) -> Box<BoostQuery> {
    let slop = terms
        .last()
        .map(|(position, _)| ((*position as i32) - 2).max(0).min(3) as u32)
        .unwrap_or(0);
    Box::new(BoostQuery::new(
        Box::new(PhraseQuery::new_with_offset_and_slop(terms, slop)),
        boost,
    ))
}

pub struct QueryOptions {
    /// single term matches in the content
    content_boost: f32,
    /// full phrase matches in the content
    content_phrase_boost: f32,
    /// single term matches in the title
    title_boost: f32,
    /// full phrase matches in the title
    title_phrase_boost: f32,
}

impl Default for QueryOptions {
    fn default() -> Self {
        QueryOptions {
            content_boost: 1.0,
            content_phrase_boost: 1.5,
            // weight title matches a little more
            title_boost: 2.0,
            title_phrase_boost: 2.5,
        }
    }
}

pub fn build_query(
    index: &Index,
    query_string: &str,
    // Applied filters
    filters: &[QueryBoost],
    // Applied boosts,
    boosts: &[QueryBoost],
    // title/content boost options
    opts: QueryOptions,
) -> (usize, BooleanQuery) {
    let schema = index.schema();
    let tokenizers = index.tokenizers();
    let fields = DocFields::as_fields();

    let content_terms = terms_for_field(&schema, tokenizers, query_string, fields.content);
    let title_terms = terms_for_field(&schema, tokenizers, query_string, fields.title);

    let term_count = content_terms.len();

    let mut term_query: QueryVec = Vec::new();

    // Boost exact matches to the full query string
    if content_terms.len() > 1 {
        // boosting phrases relative to the number of segments in a
        // continuous phrase
        let boost = opts.content_phrase_boost * content_terms.len() as f32;
        term_query.push((Occur::Should, _boosted_phrase(content_terms.clone(), boost)));
    }

    // Boost exact matches to the full query string
    if title_terms.len() > 1 {
        // boosting phrases relative to the number of segments in a
        // continuous phrase, base score higher for title
        // than content
        let boost = opts.title_phrase_boost * title_terms.len() as f32;
        term_query.push((Occur::Should, _boosted_phrase(title_terms.clone(), boost)));
    }

    for (_position, term) in content_terms {
        term_query.push((Occur::Should, _boosted_term(term, opts.content_boost)));
    }

    for (_position, term) in title_terms {
        term_query.push((Occur::Should, _boosted_term(term, opts.title_boost)));
    }

    // Boost fields that happen to have a value, such as
    // - Tags that might be represented by search terms (e.g. "repository" or "file")
    // - Certain URLs or documents we want to focus on
    for boost in boosts {
        let term = match &boost.field {
            Boost::DocId(doc_id) => {
                // Originally boosted to 3.0
                _boosted_term(Term::from_field_text(fields.id, doc_id), boost.value)
            }
            // Only considered in filters
            Boost::Favorite { .. } => continue,
            Boost::Tag(tag_id) => {
                // Defaults to 1.5
                _boosted_term(Term::from_field_u64(fields.tags, *tag_id), boost.value)
            }
            // todo: handle regex/prefixes?
            Boost::Url(url) => {
                // Originally boosted to 3.0
                _boosted_term(Term::from_field_text(fields.url, url), boost.value)
            }
        };

        term_query.push((Occur::Should, term));
    }

    // Must hit at least one of the terms
    let mut combined: QueryVec = vec![(Occur::Must, Box::new(BooleanQuery::new(term_query)))];
    // Must have one of these, will filter out stuff that doesn't
    for filter in filters {
        let term = match &filter.field {
            Boost::DocId(doc_id) => {
                // Originally boosted to 3.0
                _boosted_term(Term::from_field_text(fields.id, doc_id), 0.0)
            }
            Boost::Favorite { id, required } => {
                let occur = if *required {
                    Occur::Must
                } else {
                    Occur::Should
                };

                combined.push((
                    occur,
                    _boosted_term(Term::from_field_u64(fields.tags, *id), 3.0),
                ));

                continue;
            }
            Boost::Tag(tag_id) => {
                // Defaults to 1.5
                _boosted_term(Term::from_field_u64(fields.tags, *tag_id), 0.0)
            }
            // todo: handle regex/prefixes?
            Boost::Url(url) => {
                // Originally boosted to 3.0
                _boosted_term(Term::from_field_text(fields.url, url), 0.0)
            }
        };

        combined.push((Occur::Must, term));
    }

    (term_count, BooleanQuery::new(combined))
}

/// Helper method used to build a document query based on urls, ids or tags.
pub fn build_document_query(
    fields: DocFields,
    urls: &Vec<String>,
    ids: &Vec<String>,
    tags: &[u64],
    exclude_tags: &[u64],
) -> BooleanQuery {
    let mut term_query: QueryVec = Vec::new();
    let mut urls_query: QueryVec = Vec::new();
    let mut ids_query: QueryVec = Vec::new();

    for url in urls {
        urls_query.push((
            Occur::Should,
            _boosted_term(Term::from_field_text(fields.url, url), 0.0),
        ));
    }

    if !urls_query.is_empty() {
        term_query.push((Occur::Must, Box::new(BooleanQuery::new(urls_query))));
    }

    for id in ids {
        ids_query.push((
            Occur::Should,
            _boosted_term(Term::from_field_text(fields.id, id), 0.0),
        ));
    }

    if !ids_query.is_empty() {
        term_query.push((Occur::Must, Box::new(BooleanQuery::new(ids_query))));
    }

    for id in tags {
        term_query.push((
            Occur::Must,
            _boosted_term(Term::from_field_u64(fields.tags, *id), 0.0),
        ));
    }

    for id in exclude_tags {
        term_query.push((
            Occur::MustNot,
            _boosted_term(Term::from_field_u64(fields.tags, *id), 0.0),
        ));
    }
    BooleanQuery::new(term_query)
}

/**
 * Responsible for parsing the input query for a particular field. The tokenizer for the field
 * is used to ensure consistent tokens between indexing and queries.
 */
pub fn terms_for_field(
    schema: &Schema,
    tokenizers: &TokenizerManager,
    query: &str,
    field: Field,
) -> Vec<(usize, Term)> {
    let mut terms = Vec::new();

    let field_entry = schema.get_field_entry(field);
    let field_type = field_entry.field_type();
    if let FieldType::Str(ref str_options) = field_type {
        let option = str_options.get_indexing_options().unwrap();
        let text_analyzer = tokenizers.get(option.tokenizer()).unwrap();

        let mut token_stream = text_analyzer.token_stream(query);
        token_stream.process(&mut |token| {
            let term = Term::from_field_text(field, &token.text);
            terms.push((token.position, term));
        });
    }

    terms
}
