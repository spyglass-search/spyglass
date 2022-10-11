use std::collections::HashSet;
use std::fmt;

pub mod consts;
mod shims;

use serde::{Deserialize, Serialize};
pub use shims::*;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SearchFilter {
    // No filter
    None,
    URLRegexAllow(String),
    URLRegexSkip(String),
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

        #[no_mangle]
        pub fn search_filter() {
            STATE.with(|state| {
                let filters = state.borrow_mut().search_filter();
                let _ = object_to_stdout(&filters);
            })
        }
    };
}
pub trait SpyglassPlugin {
    /// Initial plugin load, setup any configuration you need here as well as
    /// subscribe to specific events.
    fn load(&mut self);
    /// Request plugin for updates
    fn update(&mut self, event: PluginEvent);
    /// Optional function.
    /// Only called for Lens plugins, request a set of filters to apply to a search.
    /// If not implemented, no filter is applied to the search.
    fn search_filter(&mut self) -> Vec<SearchFilter> {
        vec![SearchFilter::None]
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PluginSubscription {
    /// Check for updates at a fixed interval
    CheckUpdateInterval,
    WatchDirectory {
        path: String,
        recurse: bool,
    },
}

impl fmt::Display for PluginSubscription {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PluginSubscription::CheckUpdateInterval => {
                write!(f, "<CheckUpdateInterval>")
            }
            PluginSubscription::WatchDirectory { path, recurse } => write!(
                f,
                "<WatchDirectory {} - {}>",
                path,
                if *recurse {
                    "recursive"
                } else {
                    "non-recursive"
                }
            ),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PluginEvent {
    IntervalUpdate,
    // File watcher updates
    FileCreated(String),
    FileUpdated(String),
    FileDeleted(String),
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
    // Ask host to list the contents of a directory
    ListDir {
        path: String,
    },
    // Subscribe to PluginEvents
    Subscribe(PluginSubscription),
    // Run a sqlite query on a db file. NOTE: This is a workaround due to the fact
    // that sqlite can not be easily compiled to wasm... yet!
    SqliteQuery {
        path: String,
        query: String,
    },
    // Request mounting a file & its contents to the plugin VFS
    SyncFile {
        dst: String,
        src: String,
    },
    // Walk & enqueue the contents of a directory
    WalkAndEnqueue {
        path: String,
        extensions: HashSet<String>,
    },
}

#[derive(Deserialize, Serialize)]
pub struct ListDirEntry {
    pub path: String,
    pub is_file: bool,
    pub is_dir: bool,
}
