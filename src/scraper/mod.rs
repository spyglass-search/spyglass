#![allow(dead_code)]

mod element;
mod html;

use ego_tree::NodeRef;
use std::collections::{HashMap, HashSet};

use crate::scraper::element::Node;
use crate::scraper::html::Html;

pub struct ScrapeResult {
    pub meta: HashMap<String, String>,
    pub content: String,
}

/// Filters a DOM tree into a text document used for indexing
fn filter_text_nodes(root: &NodeRef<Node>, doc: &mut String, ignore_list: &HashSet<String>) {
    for child in root.children() {
        let node = child.value();
        if node.is_text() {
            doc.push('\n');
            doc.push_str(node.as_text().unwrap());
        } else if child.has_children() && node.is_element() {
            // Ignore certain elements
            let element = node.as_element().unwrap();
            if ignore_list.contains(&element.name()) {
                continue;
            }
            filter_text_nodes(&child, doc, ignore_list);
        }
    }
}

/// Filters a DOM tree into a text document used for indexing
pub fn html_to_text(doc: &str) -> ScrapeResult {
    // TODO: move to config file? turn into a whitelist?
    let ignore_list = HashSet::from([
        // TODO: Parse meta tags
        "head".into(),
        // Ignore elements that often don't cantain relevant info
        "header".into(),
        "footer".into(),
        "nav".into(),
        // Ignore javascript nodes
        "script".into(),
        "noscript".into(),
    ]);

    let parsed = Html::parse(doc);
    let root = parsed.tree.root();
    let meta = parsed.meta();

    let mut content = String::from("");
    filter_text_nodes(&root, &mut content, &ignore_list);

    ScrapeResult { meta, content }
}

#[cfg(test)]
mod test {
    use crate::scraper::html_to_text;

    #[test]
    fn test_html_to_text() {
        let html = include_str!("../../fixtures/raw.html");
        let doc = html_to_text(html);
        assert_eq!(doc.meta.len(), 9);
        assert!(doc.content.len() > 0);
    }
}
