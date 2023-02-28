pub mod consts;
mod shims;
pub mod utils;

use serde::{Deserialize, Serialize};
pub use shims::*;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SearchFilter {
    // No filter
    None,
    URLRegexAllow(String),
    URLRegexSkip(String),
}

/// Represents a Document in the system.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DocumentResult {
    /// The unique id of the document
    pub doc_id: String,
    /// The domain associated with the document (can be blank)
    pub domain: String,
    /// The title associated with the document
    pub title: String,
    /// The description of the document
    pub description: String,
    /// The url for the document
    pub url: String,
    /// The tags associated with the document
    pub tags: Vec<Tag>,
}

#[macro_export]
macro_rules! register_plugin {
    ($t:ty) => {
        thread_local! {
            static STATE: std::cell::RefCell<$t> = std::cell::RefCell::new(Default::default());

        }

        fn main() {
            STATE.with(|state| {
                state.borrow_mut().load();
            });
        }

        #[no_mangle]
        pub fn update() {
            STATE.with(|state| {
                let event = $crate::object_from_stdin::<PluginEvent>();
                if let Ok(event) = event {
                    state.borrow_mut().update(event);
                }
            })
        }
    };
}
pub trait SpyglassPlugin {
    /// Initial plugin load, setup any configuration you need here as well as
    /// subscribe to specific events.
    fn load(&mut self);
    /// Asynchronous updates for plugin events
    fn update(&mut self, event: PluginEvent);
}

/// Event providing the plugin asynchronous data
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PluginEvent {
    /// A page of documents
    DocumentResponse {
        request_id: String,
        page_count: u32,
        page: u32,
        documents: Vec<DocumentResult>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PluginCommandRequest {
    DeleteDoc {
        url: String,
    },
    // Enqueue a list of URLs into the crawl queue
    Enqueue {
        urls: Vec<String>,
    },
    // Requests a set of documents that match the
    // provided query. If subscribe is set to false
    // The query is a one time query. If set to true
    // the query will be run once then every minute after
    // that
    QueryDocuments {
        query: DocumentQuery,
        subscribe: bool,
    },
    // Request to modify the tags for all documents that
    // match the associated document query.
    ModifyTags {
        documents: DocumentQuery,
        tag_modifications: TagModification,
    },
}

#[derive(Deserialize, Serialize)]
pub struct ListDirEntry {
    pub path: String,
    pub is_file: bool,
    pub is_dir: bool,
}

pub type Tag = (String, String);

/// Defines a Tag modification request. Tags can be added or deleted
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct TagModification {
    pub add: Option<Vec<Tag>>,
    pub remove: Option<Vec<Tag>>,
}

/// Defines a document query.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct DocumentQuery {
    /// Will match against the urls. Since a single document can only
    /// have one url these fields are or'd together
    pub urls: Option<Vec<String>>,
    /// With match against the document id. Since a single document can
    /// only have one document id these fields are or'd together
    pub ids: Option<Vec<String>>,
    /// Matches only documents that have the specified tags. These entries
    /// are and'd together
    pub has_tags: Option<Vec<Tag>>,
    /// Matches only documents that do not have the specified tags. These
    /// entries are and'd together
    pub exclude_tags: Option<Vec<Tag>>,
}
