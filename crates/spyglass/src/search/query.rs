use tantivy::query::{BooleanQuery, BoostQuery, Occur, Query, TermQuery};
use tantivy::schema::*;
use tantivy::Score;

use super::DocFields;

type QueryVec = Vec<(Occur, Box<dyn Query>)>;

fn _boosted_term(field: Field, term: &str, boost: Score) -> Box<BoostQuery> {
    Box::new(BoostQuery::new(
        Box::new(TermQuery::new(
            Term::from_field_text(field, term),
            // Needs WithFreqs otherwise scoring is wonky.
            IndexRecordOption::WithFreqs,
        )),
        boost,
    ))
}

pub fn build_query(fields: DocFields, query_string: &str) -> BooleanQuery {
    // Tokenize query string
    let query_string = query_string.to_lowercase();
    let terms: Vec<&str> = query_string
        .split(' ')
        .into_iter()
        .map(|token| token.trim())
        .collect();

    let mut term_query: QueryVec = Vec::new();
    // Boost exact matches to the full query string
    if terms.len() > 1 {
        term_query.push((
            Occur::Should,
            _boosted_term(fields.title, &query_string, 5.0),
        ));
        term_query.push((
            Occur::Should,
            _boosted_term(fields.content, &query_string, 5.0),
        ));
    }

    for term in terms {
        // Emphasize matches in the title more than words in the content
        term_query.push((Occur::Should, _boosted_term(fields.content, term, 1.0)));
        term_query.push((Occur::Should, _boosted_term(fields.title, term, 5.0)));
    }

    BooleanQuery::new(vec![(Occur::Must, Box::new(BooleanQuery::new(term_query)))])
}
