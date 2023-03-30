pub mod consts;
mod shims;
pub mod utils;

use serde::{Deserialize, Serialize};
use serde_json;
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HttpResponse {
    pub headers: Vec<(String, String)>,
    pub response: Option<String>,
}

impl HttpResponse {
    pub fn as_json(&self) -> Option<serde_json::Value> {
        if let Some(val) = &self.response {
            return serde_json::from_str(val.as_str()).ok();
        }
        None
    }
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
    // Periodic update for request for a plugin
    IntervalUpdate,
    HttpResponse {
        url: String,
        result: Result<HttpResponse, String>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Authentication {
    BASIC(String, Option<String>),
    BEARER(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
    OPTIONS,
    PATCH,
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
    // Request to add one or more documents to the index.
    AddDocuments {
        documents: Vec<DocumentUpdate>,
        // Tags to apply to all documents in this set
        tags: Vec<Tag>,
    },
    // Requests that the plugin be called at a regular interval
    // (currently every 10 minutes) to allow the plugin to
    // process an new updates.
    SubscribeForUpdates,
    // Request an http resource.
    HttpRequest {
        headers: Vec<(String, String)>,
        method: HttpMethod,
        url: String,
        body: Option<String>,
        auth: Option<Authentication>,
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

/// Defines a document update.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct DocumentUpdate {
    /// Text content from page after stripping content that should not be
    /// indexed. For HTML this is HTML tags & semantically unimportant sections
    /// (header/footer/etc.)
    pub content: Option<String>,
    /// Short description of the content. Terms found in the description are
    /// boosted above terms found in content
    pub description: Option<String>,
    /// The title of the document. Terms found in the title are boosted above description
    /// terms and content terms
    pub title: Option<String>,
    /// Uniquely identifying URL for this document.
    pub url: String,
    /// URL used to open the document in finder/web browser/etc.
    pub open_url: Option<String>,
    /// Tags to apply to this document
    pub tags: Vec<Tag>,
}
