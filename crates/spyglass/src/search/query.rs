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

pub fn build_query(
    schema: Schema,
    tokenizers: TokenizerManager,
    fields: DocFields,
    query_string: &str,
    applied_lenses: &Vec<u64>,
) -> BooleanQuery {
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

    for id in applied_lenses {
        term_query.push((
            Occur::Must,
            _boosted_term(Term::from_field_u64(fields.tags, *id), 1.0),
        ))
    }

    BooleanQuery::new(vec![(Occur::Must, Box::new(BooleanQuery::new(term_query)))])
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
