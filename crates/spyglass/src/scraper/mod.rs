#![allow(dead_code)]

mod element;
mod html;

use ego_tree::NodeRef;
use html5ever::QualName;
use std::collections::{HashMap, HashSet};
use url::Url;

use crate::scraper::element::Node;
use crate::scraper::html::Html;

pub const DEFAULT_DESC_LENGTH: usize = 256;

#[derive(Debug)]
pub struct ScrapeResult {
    pub title: Option<String>,
    /// Page description, extracted from meta tags or summarized from the actual content
    pub description: String,
    pub meta: HashMap<String, String>,
    pub content: String,
    pub links: HashSet<String>,
    /// Index should use this URL instead of the one that lead to the content.
    pub canonical_url: Option<Url>,
}

/// Walk the DOM and grab all the p nodes
fn filter_p_nodes(root: &NodeRef<Node>, p_list: &mut Vec<String>) {
    for child in root.children() {
        let node = child.value();
        if node.is_element() {
            let element = node.as_element().expect("Expected node to be element");
            if element.name().eq_ignore_ascii_case("p") {
                let mut p_content = String::from("");
                let mut links = HashSet::new();
                filter_text_nodes(&child, &mut p_content, &mut links);

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
            doc.push_str(node.as_text().expect("Expected text node"));
        } else if node.is_element() {
            // Ignore elements on the ignore list
            let element = node.as_element().expect("Expected element node");
            if ignore_list.contains(&element.name()) {
                continue;
            }

            // Ignore elements whose role is "navigation"
            // TODO: Filter out full-list of ARIA roles that are not content
            if element.attrs.contains_key(&role_key)
                && (element
                    .attrs
                    .get(&role_key)
                    .expect("Expected role_key")
                    .to_string()
                    == *"navigation"
                    || element
                        .attrs
                        .get(&role_key)
                        .expect("Expected role_key")
                        .to_string()
                        == *"contentinfo"
                    || element
                        .attrs
                        .get(&role_key)
                        .expect("Expected role_key")
                        .to_string()
                        == *"button")
            {
                continue;
            }

            // Save links
            if element.name() == "a" && element.attrs.contains_key(&href_key) {
                let href = element
                    .attrs
                    .get(&href_key)
                    .expect("Expected href_key")
                    .to_string();
                // Ignore anchor links
                if !href.starts_with('#') {
                    links.insert(href.to_string());
                }
            } else if element.name() == "br" && !doc.ends_with(' ') {
                doc.push(' ');
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
    // Meta tags
    let meta = parsed.meta();
    let link_tags = parsed.link_tags();
    // Content
    let title = parsed.title();
    let mut content = String::from("");
    let mut links = HashSet::new();
    filter_text_nodes(&root, &mut content, &mut links);
    content = content.trim().to_string();

    let mut description = if let Some(desc) = meta.get("description") {
        desc.to_owned()
    } else if let Some(desc) = meta.get("og:description") {
        desc.to_owned()
    } else {
        "".to_string()
    };

    if description.is_empty() && !content.is_empty() {
        // Extract first paragraph from content w/ text to use as the description
        let mut p_list = Vec::new();
        filter_p_nodes(&root, &mut p_list);

        let text = p_list.iter().find(|p_content| !p_content.trim().is_empty());
        if text.is_some() && !text.unwrap().is_empty() {
            description = text.unwrap_or(&String::from("")).trim().to_owned()
        } else if !content.is_empty() {
            // Still nothing? Grab the first 256 words-ish
            description = content
                .split(' ')
                .take(DEFAULT_DESC_LENGTH)
                .collect::<Vec<&str>>()
                .join(" ")
        }
    }

    // If there's a canonical URL on this page, attempt to determine whether it's valid.
    // More info about canonical URLS:
    // https://developers.google.com/search/docs/advanced/crawling/consolidate-duplicate-urls
    let canonical_url = match link_tags.get("canonical").map(|x| Url::parse(x)) {
        // Canonical URLs *must* be a full, valid URL
        Some(Ok(mut parsed)) => {
            // Ignore fragments
            parsed.set_fragment(None);
            Some(parsed)
        }
        _ => None,
    };

    ScrapeResult {
        canonical_url,
        content,
        description,
        links,
        meta,
        title,
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
        assert!(!doc.content.is_empty());
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
        assert_eq!(doc.description, "Rust is a multi-paradigm, general-purpose programming language designed for performance and safety, especially safe concurrency. Rust is syntactically similar to C++, but can guarantee memory safety by using a borrow checker to validate references. Rust achieves memory safety without garbage collection, and reference counting is optional. Rust has been called a systems programming language, and in addition to high-level features such as functional programming it also offers mechanisms for low-level memory management.");

        let html = include_str!("../../../../fixtures/html/personal_blog.html");
        let doc = html_to_text(html);
        // ugh need to fix this
        assert_eq!(doc.description, "2020 July 15 - San Francisco |  855 words");
    }

    #[test]
    fn test_description_extraction_yc() {
        let html = include_str!("../../../../fixtures/html/summary_test.html");
        let doc = html_to_text(html);

        assert_eq!(doc.title.unwrap(), "Why YC");
        assert_eq!(doc.description, "March 2006, rev August 2009 Yesterday one of the founders we funded asked me why we started Y Combinator.  Or more precisely, he asked if we'd started YC mainly for fun. Kind of, but not quite.  It is enormously fun to be able to work with Rtm and Trevor again.  I missed that after we sold Viaweb, and for all the years after I always had a background process running, looking for something we could do together.  There is definitely an aspect of a band reunion to Y Combinator.  Every couple days I slip and call it \"Viaweb.\" Viaweb we started very explicitly to make money.  I was sick of living from one freelance project to the next, and decided to just work as hard as I could till I'd made enough to solve the problem once and for all.  Viaweb was sometimes fun, but it wasn't designed for fun, and mostly it wasn't.  I'd be surprised if any startup is. All startups are mostly schleps. The real reason we started Y Combinator is neither selfish nor virtuous.  We didn't start it mainly to make money; we have no idea what our average returns might be, and won't know for years.  Nor did we start YC mainly to help out young would-be founders, though we do like the idea, and comfort ourselves occasionally with the thought that if all our investments tank, we will thus have been doing something unselfish.  (It's oddly nondeterministic.) The real");
    }
}
