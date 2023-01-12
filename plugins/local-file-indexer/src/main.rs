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
        // List of supported file types
        let default_exts =
            HashSet::from_iter(vec!["md".to_string(), "txt".to_string()].into_iter());
        self.extensions = if let Ok(blob) = std::env::var(EXTS_LIST_ENV) {
            if let Ok(exts) = serde_json::from_str(&blob) {
                exts
            } else {
                default_exts
            }
        } else {
            default_exts
        };

        // When paths were last synced
        self.last_synced = if let Ok(blob) = std::fs::read_to_string(PLUGIN_DATA) {
            serde_json::from_str::<SyncData>(&blob).map_or(Default::default(), |x| x)
        } else {
            Default::default()
        };

        let paths = if let Ok(blob) = std::env::var(FOLDERS_LIST_ENV) {
            serde_json::from_str::<Vec<String>>(&blob).map_or(Vec::new(), |x| x)
        } else {
            Vec::new()
        };

        for path in paths.iter().map(|path| Path::new(&path).to_path_buf()) {
            let now = Utc::now();

            let last_processed_time = self
                .last_synced
                .path_to_times
                .entry(path.to_path_buf())
                .or_default();

            let diff = now - *last_processed_time;
            if diff.num_days() > 1 {
                if let Err(e) = walk_and_enqueue_dir(path.to_path_buf(), &self.extensions) {
                    log(format!("Unable to process dir: {e}"));
                } else {
                    *last_processed_time = now;
                }
            }

            // List to notifications
            subscribe(PluginSubscription::WatchDirectory {
                path: path.to_path_buf(),
                recurse: true,
            });
        }

        // Save list of processed paths to data dir
        if let Ok(blob) = serde_json::to_string_pretty(&self.last_synced) {
            let _ = std::fs::write(PLUGIN_DATA, blob);
        }
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
