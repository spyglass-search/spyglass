#![allow(dead_code)]

mod element;
mod html;

use ego_tree::NodeRef;
use html5ever::QualName;
use std::collections::{HashMap, HashSet};

use crate::scraper::element::Node;
use crate::scraper::html::Html;

#[derive(Debug)]
pub struct ScrapeResult {
    pub title: Option<String>,
    // Page description, extracted from meta tags or summarized from the actual content
    pub description: String,
    pub meta: HashMap<String, String>,
    pub content: String,
    pub links: HashSet<String>,
}

/// Walk the DOM and grab all the p nodes
fn filter_p_nodes(root: &NodeRef<Node>, p_list: &mut Vec<String>) {
    for child in root.children() {
        let node = child.value();
        if node.is_element() {
            let element = node.as_element().unwrap();
            if element.name().eq_ignore_ascii_case("p") {
                let mut p_content = String::from("");
                let mut _links = HashSet::new();
                filter_text_nodes(&child, &mut p_content, &mut _links);

                if !p_content.is_empty() {
                    p_list.push(p_content);
                }
            }
        }

        if child.has_children() {
            filter_p_nodes(&child, p_list);
        }
    }
}

/// Filters a DOM tree into a text document used for indexing
fn filter_text_nodes(root: &NodeRef<Node>, doc: &mut String, links: &mut HashSet<String>) {
    // TODO: move to config file? turn into a whitelist?
    // TODO: Ignore list could also be updated per domain as well if needed
    let ignore_list: HashSet<String> = HashSet::from([
        "head".into(),
        "sup".into(),
        // Ignore elements that often don't contain relevant info
        "header".into(),
        "footer".into(),
        "nav".into(),
        // form elements
        "label".into(),
        "textarea".into(),
        // Ignore javascript/style nodes
        "script".into(),
        "noscript".into(),
        "style".into(),
    ]);

    let href_key = QualName::new(None, ns!(), local_name!("href"));
    let role_key = QualName::new(None, ns!(), local_name!("role"));

    for child in root.children() {
        let node = child.value();
        if node.is_text() {
            doc.push_str(node.as_text().unwrap());
        } else if node.is_element() {
            // Ignore elements on the ignore list
            let element = node.as_element().unwrap();
            if ignore_list.contains(&element.name()) {
                continue;
            }

            // Ignore elements whose role is "navigation"
            // TODO: Filter out full-list of ARIA roles that are not content
            if element.attrs.contains_key(&role_key)
                && (element.attrs.get(&role_key).unwrap().to_string() == *"navigation"
                    || element.attrs.get(&role_key).unwrap().to_string() == *"contentinfo"
                    || element.attrs.get(&role_key).unwrap().to_string() == *"button")
            {
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
                filter_text_nodes(&child, doc, links);
                // Add spacing to elements that naturally have spacing
                if element.name().eq_ignore_ascii_case("p")
                    || element.name().eq_ignore_ascii_case("h1")
                    || element.name().eq_ignore_ascii_case("h2")
                    || element.name().eq_ignore_ascii_case("h3")
                    || element.name().eq_ignore_ascii_case("h4")
                    || element.name().eq_ignore_ascii_case("h5")
                {
                    doc.push(' ');
                }
            }
        }
    }
}

/// Filters a DOM tree into a text document used for indexing
pub fn html_to_text(doc: &str) -> ScrapeResult {
    let parsed = Html::parse(doc);
    let root = parsed.tree.root();
    let meta = parsed.meta();
    let title = parsed.title();

    let mut content = String::from("");
    let mut links = HashSet::new();
    filter_text_nodes(&root, &mut content, &mut links);

    let description = {
        if meta.contains_key("description") {
            meta.get("description").unwrap().to_string()
        } else if meta.contains_key("og:description") {
            meta.get("og:description").unwrap().to_string()
        } else if !content.is_empty() {
            // Extract first paragraph from content w/ text to use as the description
            let mut p_list = Vec::new();
            filter_p_nodes(&root, &mut p_list);

            let text = p_list.iter().find(|content| !content.is_empty());

            text.unwrap_or(&String::from("")).trim().to_owned()
        } else {
            "".to_string()
        }
    };

    ScrapeResult {
        title,
        description,
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
        let html = include_str!("../../../../fixtures/html/raw.html");
        let doc = html_to_text(html);
        assert_eq!(doc.title, Some("Old School RuneScape Wiki".to_string()));
        assert_eq!(doc.meta.len(), 9);
        assert!(doc.content.len() > 0);
        assert_eq!(doc.links.len(), 58);
    }

    #[test]
    fn test_description_extraction() {
        let html = include_str!("../../../../fixtures/html/wikipedia_entry.html");
        let doc = html_to_text(html);

        assert_eq!(
            doc.title.unwrap(),
            "Rust (programming language) - Wikipedia"
        );
        assert_eq!(doc.description, "Rust is a multi-paradigm, general-purpose programming language designed for performance and safety, especially safe concurrency. Rust is syntactically similar to C++, but can guarantee memory safety by using a borrow checker to validate references. Rust achieves memory safety without garbage collection, and reference counting is optional. Rust has been called a systems programming language, and in addition to high-level features such as functional programming it also offers mechanisms for low-levelmemory management.");

        let html = include_str!("../../../../fixtures/html/personal_blog.html");
        let doc = html_to_text(html);
        // ugh need to fix this
        assert_eq!(doc.description, "2020 July 15 - San Francisco |855 words");
    }
}
