use std::collections::HashSet;

use tantivy::{
    fastfield::MultiValuedFastFieldReader, termdict::TermDictionary, tokenizer::TextAnalyzer, DocId,
};

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

#[allow(dead_code)]
pub fn ff_to_string(
    doc_id: DocId,
    reader: &MultiValuedFastFieldReader<u64>,
    terms: &TermDictionary,
) -> Option<String> {
    let mut vals = Vec::new();
    reader.get_vals(doc_id, &mut vals);

    if let Some(term_id) = vals.pop() {
        let mut bytes = Vec::new();
        if terms.ord_to_term(term_id, &mut bytes).is_err() {
            return None;
        }

        return String::from_utf8(bytes.to_vec()).ok();
    }

    None
}

/// Creates a short preview from content based on the search query terms by
/// finding matches for words and creating a window around each match, joining
/// together overlaps & returning the final string.
pub fn generate_highlight_preview(tokenizer: &TextAnalyzer, query: &str, content: &str) -> String {
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

#[cfg(test)]
mod test {
    use crate::search::utils::generate_highlight_preview;
    use crate::search::{IndexPath, Searcher};
    use entities::schema::DocFields;
    use entities::schema::SearchDocument;

    #[test]
    fn test_find_highlights() {
        let searcher =
            Searcher::with_index(&IndexPath::Memory, false).expect("Unable to open index");
        let blurb = r#"Rust rust is a multi-paradigm, high-level, general-purpose programming"#;

        let fields = DocFields::as_fields();
        let tokenizer = searcher
            .index
            .tokenizer_for_field(fields.content)
            .expect("Unable to get tokenizer for content field");
        let desc = generate_highlight_preview(&tokenizer, "rust programming", &blurb);
        assert_eq!(desc, "<span><mark>Rust</mark> <mark>rust</mark> is a multi-paradigm, high-level, general-purpose <mark>programming</mark> ...</span>");
    }
}
