use std::collections::HashMap;

use tantivy::schema::*;
use tantivy::query::{BooleanQuery, Occur, Query, TermQuery};

use super::DocFields;
use crate::config::Lens;

type QueryVec = Vec<(Occur, Box<dyn Query>)>;

pub fn build_query(
    fields: DocFields,
    lenses: &HashMap<String, Lens>,
    applied_lens: &[String],
    query_string: &str,
) -> BooleanQuery {
    // Tokenize query string
    let terms: Vec<&str> = query_string.split(' ')
        .into_iter()
        .map(|token| {
            token.trim()
        })
        .collect();

    log::info!("lenses: {:?}, terms: {:?}", applied_lens, terms);

    let mut lense_queries: QueryVec = Vec::new();
    for lens in applied_lens {
        if lenses.contains_key(lens) {
            let lens = lenses.get(lens).unwrap();
            for domain in &lens.domains {
                lense_queries.push((
                    Occur::Should,
                    Box::new(TermQuery::new(
                        Term::from_field_text(fields.domain, domain),
                        IndexRecordOption::Basic,
                    )),
                ));
            }
        }
    }

    let mut term_query: QueryVec = Vec::new();
    for term in terms {
        term_query.push((
            Occur::Should,
            Box::new(TermQuery::new(
                Term::from_field_text(fields.content, term),
                IndexRecordOption::Basic,
            )),
        ));

        term_query.push((
            Occur::Should,
            Box::new(TermQuery::new(
                Term::from_field_text(fields.title, term),
                IndexRecordOption::Basic,
            )),
        ));
    }

    let mut nested_query: QueryVec =
        vec![(Occur::Must, Box::new(BooleanQuery::new(term_query)))];
    if !lense_queries.is_empty() {
        nested_query.push((Occur::Must, Box::new(BooleanQuery::new(lense_queries))));
    }

    BooleanQuery::new(nested_query)
}