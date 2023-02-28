use tantivy::query::{BooleanQuery, BoostQuery, Occur, PhraseQuery, Query, TermQuery};
use tantivy::schema::*;
use tantivy::tokenizer::TokenizerManager;
use tantivy::Score;

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

fn _boosted_phrase(terms: Vec<Term>, boost: Score) -> Box<BoostQuery> {
    Box::new(BoostQuery::new(Box::new(PhraseQuery::new(terms)), boost))
}

pub fn build_query<I>(
    schema: Schema,
    tokenizers: TokenizerManager,
    fields: DocFields,
    query_string: &str,
    // Applied filters
    applied_lenses: &Vec<u64>,
    // Boosts based on implicit/explicit tag detection
    tag_boosts: I,
    // Id of favorited boost
    favorite_boost: Option<i64>,
) -> BooleanQuery
where
    I: Iterator<Item = i64>,
{
    let content_terms = terms_for_field(&schema, &tokenizers, query_string, fields.content);
    let title_terms: Vec<Term> = terms_for_field(&schema, &tokenizers, query_string, fields.title);

    let mut term_query: QueryVec = Vec::new();

    // Boost exact matches to the full query string
    if content_terms.len() > 1 {
        // boosting phrases relative to the number of segments in a
        // continuous phrase
        let boost = 2.0 * content_terms.len() as f32;
        term_query.push((Occur::Should, _boosted_phrase(content_terms.clone(), boost)));
    }

    // Boost exact matches to the full query string
    if title_terms.len() > 1 {
        // boosting phrases relative to the number of segments in a
        // continuous phrase, base score higher for title
        // than content
        let boost = 2.5 * title_terms.len() as f32;
        term_query.push((Occur::Should, _boosted_phrase(title_terms.clone(), boost)));
    }

    for term in content_terms {
        term_query.push((Occur::Should, _boosted_term(term, 1.0)));
    }

    for term in title_terms {
        term_query.push((Occur::Should, _boosted_term(term, 2.0)));
    }

    // Tags that might be represented by search terms (e.g. "repository" or "file")
    for tag_id in tag_boosts {
        term_query.push((
            Occur::Should,
            _boosted_term(Term::from_field_u64(fields.tags, tag_id as u64), 1.5),
        ))
    }

    let mut combined: QueryVec = vec![(Occur::Must, Box::new(BooleanQuery::new(term_query)))];

    for id in applied_lenses {
        combined.push((
            Occur::Must,
            _boosted_term(Term::from_field_u64(fields.tags, *id), 0.0),
        ));
    }

    // Greatly boost content that have our terms + a favorite.
    if let Some(favorite_boost) = favorite_boost {
        combined.push((
            Occur::Should,
            _boosted_term(
                Term::from_field_u64(fields.tags, favorite_boost as u64),
                3.0,
            ),
        ));
    }

    BooleanQuery::new(combined)
}

/// Helper method used to build a document query based on urls, ids or tags.
pub fn build_document_query(
    fields: DocFields,
    urls: &Vec<String>,
    ids: &Vec<String>,
    tags: &Vec<u64>,
    exclude_tags: &Vec<u64>,
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
fn terms_for_field(
    schema: &Schema,
    tokenizers: &TokenizerManager,
    query: &str,
    field: Field,
) -> Vec<Term> {
    let mut terms = Vec::new();

    let field_entry = schema.get_field_entry(field);
    let field_type = field_entry.field_type();
    if let FieldType::Str(ref str_options) = field_type {
        let option = str_options.get_indexing_options().unwrap();
        let text_analyzer = tokenizers.get(option.tokenizer()).unwrap();

        let mut token_stream = text_analyzer.token_stream(query);
        token_stream.process(&mut |token| {
            let term = Term::from_field_text(field, &token.text);
            terms.push(term);
        });
    }

    terms
}
