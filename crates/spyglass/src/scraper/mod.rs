#![allow(dead_code)]

mod element;
mod html;

use ego_tree::NodeRef;
use html5ever::QualName;
use std::collections::{HashMap, HashSet};

use crate::scraper::element::Node;
use crate::scraper::html::Html;

pub struct ScrapeResult {
    pub title: Option<String>,
    pub meta: HashMap<String, String>,
    pub content: String,
    pub links: HashSet<String>,
}

/// Filters a DOM tree into a text document used for indexing
fn filter_text_nodes(
    root: &NodeRef<Node>,
    doc: &mut String,
    links: &mut HashSet<String>,
    ignore_list: &HashSet<String>,
) {
    let href_key = QualName::new(None, ns!(), local_name!("href"));

    for child in root.children() {
        let node = child.value();
        if node.is_text() {
            doc.push('\n');
            doc.push_str(node.as_text().unwrap());
        } else if node.is_element() {
            // Ignore elements on the ignore list
            let element = node.as_element().unwrap();
            if ignore_list.contains(&element.name()) {
                continue;
            }

            // Save links
            if element.name() == "a" && element.attrs.contains_key(&href_key) {
                let href = element.attrs.get(&href_key).unwrap().to_string();
                // Ignore anchor links
                if !href.starts_with('#') {
                    links.insert(href);
                }
            }

            if child.has_children() {
                filter_text_nodes(&child, doc, links, ignore_list);
            }
        }
    }
}

/// Filters a DOM tree into a text document used for indexing
pub fn html_to_text(doc: &str) -> ScrapeResult {
    // TODO: move to config file? turn into a whitelist?
    // TODO: Ignore list could also be updated per domain as well if needed
    let ignore_list = HashSet::from([
        // TODO: Parse meta tags
        "head".into(),
        // Ignore elements that often don't contain relevant info
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
    let title = parsed.title();

    let mut content = String::from("");
    let mut links = HashSet::new();
    filter_text_nodes(&root, &mut content, &mut links, &ignore_list);

    ScrapeResult {
        title,
        meta,
        content,
        links,
    }
}

#[cfg(test)]
mod test {
    use crate::scraper::html_to_text;

    #[test]
    fn test_html_to_text() {
        let html = include_str!("../../../../fixtures/raw.html");
        let doc = html_to_text(html);
        assert_eq!(doc.title, Some("Old School RuneScape Wiki".to_string()));
        assert_eq!(doc.meta.len(), 9);
        assert!(doc.content.len() > 0);
        assert_eq!(doc.links.len(), 58);
    }
}
