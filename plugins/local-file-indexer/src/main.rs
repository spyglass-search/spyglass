use chrono::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use spyglass_plugin::utils::path_to_uri;
use spyglass_plugin::*;

#[derive(Default)]
struct Plugin {
    extensions: HashSet<String>,
    last_synced: SyncData,
}

const PLUGIN_DATA: &str = "/data.json";
const FOLDERS_LIST_ENV: &str = "FOLDERS_LIST";
const EXTS_LIST_ENV: &str = "EXTS_LIST";

#[derive(Default, Deserialize, Serialize)]
struct SyncData {
    path_to_times: HashMap<PathBuf, DateTime<Utc>>,
}

register_plugin!(Plugin);

impl SpyglassPlugin for Plugin {
    fn load(&mut self) {
        // Noop, now handled internally. Plugin can be removed when settings
        // are converted to core
    }

    fn update(&mut self, event: PluginEvent) {
        match event {
            PluginEvent::FileCreated(path) | PluginEvent::FileUpdated(path) => {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if self.extensions.contains(ext) {
                        enqueue_all(&[path_to_uri(path)])
                    }
                }
            }
            PluginEvent::FileDeleted(path) => delete_doc(&path_to_uri(path)),
            _ => {}
        }
    }

    fn search_filter(&mut self) -> Vec<SearchFilter> {
        vec![SearchFilter::URLRegexAllow("file://.*".to_string())]
    }
}
