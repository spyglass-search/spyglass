use std::collections::HashSet;
use std::path::Path;

use spyglass_plugin::*;
use spyglass_plugin::utils::path_to_uri;

#[derive(Default)]
struct Plugin {
    extensions: HashSet<String>,
    processed_paths: HashSet<String>,
}

const PLUGIN_DATA: &str = "/data.json";
const FOLDERS_LIST_ENV: &str = "FOLDERS_LIST";

register_plugin!(Plugin);

impl SpyglassPlugin for Plugin {
    fn load(&mut self) {
        // TODO: Make this configurable.
        self.extensions = HashSet::from_iter(vec!["md".to_string(), "txt".to_string()].into_iter());
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
                enqueue_all(&[path_to_uri(Path::new(&path).to_path_buf())])
            }
            PluginEvent::FileDeleted(path) => delete_doc(&path_to_uri(Path::new(&path).to_path_buf())),
            _ => {}
        }
    }

    fn search_filter(&mut self) -> Vec<SearchFilter> {
        vec![SearchFilter::URLRegexAllow("file://.*".to_string())]
    }
}
