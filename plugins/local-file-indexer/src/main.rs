use std::collections::HashSet;

use spyglass_plugin::utils::path_to_uri;
use spyglass_plugin::*;

#[derive(Default)]
struct Plugin {
    extensions: HashSet<String>,
    processed_paths: HashSet<String>,
}

const PLUGIN_DATA: &str = "/data.json";
const FOLDERS_LIST_ENV: &str = "FOLDERS_LIST";
const EXTS_LIST_ENV: &str = "EXTS_LIST";

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

        // List of paths being checked.
        self.processed_paths = match std::fs::read_to_string(PLUGIN_DATA) {
            Ok(blob) => {
                if let Ok(paths) = serde_json::from_str::<HashSet<String>>(&blob) {
                    paths
                } else {
                    HashSet::new()
                }
            }
            Err(_) => HashSet::new(),
        };

        let paths = if let Ok(blob) = std::env::var(FOLDERS_LIST_ENV) {
            if let Ok(folders) = serde_json::from_str::<Vec<String>>(&blob) {
                folders
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        for path in paths {
            // Have we processed this directory?
            if !self.processed_paths.contains(&path) {
                if let Err(e) = walk_and_enqueue_dir(&path, &self.extensions) {
                    log(format!("Unable to process dir: {}", e));
                } else {
                    self.processed_paths.insert(path.to_string());
                }
            }

            // List to notifications
            subscribe(PluginSubscription::WatchDirectory {
                path: path.to_string(),
                recurse: true,
            });
        }

        // Save list of processed paths to data dir
        if let Ok(blob) = serde_json::to_string_pretty(&self.processed_paths) {
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
