use spyglass_plugin::*;
use std::collections::HashSet;
use std::path::Path;

#[derive(Default)]
struct Plugin {
    extensions: HashSet<String>,
    processed_paths: HashSet<String>,
}

const PLUGIN_DATA: &str = "/data.json";
const FOLDERS_LIST_ENV: &str = "FOLDERS_LIST";

register_plugin!(Plugin);

// Create a file URI
fn to_uri(path: &str) -> String {
    let host = "Lord-of-Data.local";
    format!("file://{}/{}", host, path)
}

impl Plugin {
    fn walk_and_enqueue(&self, path: &str) {
        if let Ok(folder_entries) = list_dir(path) {
            let mut filtered = Vec::new();
            // Filter out only files that match our extension list
            for entry in folder_entries {
                if entry.is_dir {
                    self.walk_and_enqueue(&entry.path);
                } else {
                    let path = Path::new(&entry.path);
                    if let Some(ext) = path
                        .extension()
                        .and_then(|x| x.to_str())
                        .map(|x| x.to_string())
                    {
                        if self.extensions.contains(&ext) {
                            filtered.push(to_uri(&entry.path));
                        }
                    }
                }
            }

            // Add to crawl_queue & mark as processed
            if !filtered.is_empty() {
                enqueue_all(&filtered);
            }
        }
    }
}

impl SpyglassPlugin for Plugin {
    fn load(&mut self) {
        self.extensions = HashSet::from_iter(vec!["md".to_string(), "txt".to_string()].into_iter());
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
                self.walk_and_enqueue(&path);
                self.processed_paths.insert(path.to_string());
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
                enqueue_all(&[to_uri(&path)])
            }
            PluginEvent::FileDeleted(path) => delete_doc(&to_uri(&path)),
            _ => {}
        }
    }
}
