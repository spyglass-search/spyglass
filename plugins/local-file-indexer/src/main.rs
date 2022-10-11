use spyglass_plugin::*;
use std::collections::HashSet;
use url::Url;

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
    // Eventually this will be away to keep track of multiple devices and searching across
    // them. Might make sense to generate a UUID and assign to this computer(?) hostname
    // can be changed by the user.
    let host = if let Ok(hname) = std::env::var("HOST_NAME") {
        hname
    } else {
        "home.local".into()
    };

    let mut new_url = Url::parse("file://").expect("Base URI");
    let _ = new_url.set_host(Some(&host));
    // Fixes issues handling windows drive letters
    new_url.set_path(&path.replace(':', "%3A"));
    new_url.to_string()
}

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

        let exts: Vec<String> = self.extensions.iter().map(|x| x.to_owned()).collect();

        for path in paths {
            // Have we processed this directory?
            if !self.processed_paths.contains(&path) {
                if let Err(e) = walk_and_enqueue_dir(&path, &exts) {
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
                enqueue_all(&[to_uri(&path)])
            }
            PluginEvent::FileDeleted(path) => delete_doc(&to_uri(&path)),
            _ => {}
        }
    }

    fn search_filter(&mut self) -> Vec<SearchFilter> {
        vec![SearchFilter::URLRegexAllow("file://.*".to_string())]
    }
}
