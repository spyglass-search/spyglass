use tantivy::{DocId, fastfield::MultiValuedFastFieldReader, termdict::TermDictionary};

pub fn ff_to_string(doc_id: DocId, reader: &MultiValuedFastFieldReader<u64>, terms: &TermDictionary) -> Option<String> {
    let mut vals = Vec::new();
    reader.get_vals(doc_id, &mut vals);

    if let Some(term_id) = vals.pop() {
        let mut bytes = Vec::new();
        if let Err(_) = terms.ord_to_term(term_id, &mut bytes) {
            return None;
        }

        return String::from_utf8(bytes.to_vec()).ok()
    }

    None
}