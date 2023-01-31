use serde_json::Value;
use spyglass_plugin::*;
use std::{fs, path::Path};

const DATA_DIR: &str = "/";
const BOOKMARK_FILE: &str = "Bookmarks";

#[derive(Default)]
struct Plugin;

register_plugin!(Plugin);

fn parse_children(children: &Value, to_add: &mut Vec<String>) {
    if let Some(children) = children.as_array() {
        for child in children {
            let child_type = &child["type"];
            if child_type.is_null() || !child_type.is_string() {
                continue;
            }

            match child_type.as_str() {
                Some("url") => {
                    // Ignore invalid URLs
                    if !child["url"].is_string() {
                        continue;
                    }

                    if let Some(url) = child["url"].as_str() {
                        to_add.push(url.into());
                    }
                }
                // Recurse through folders to find more bookmarks
                Some("folder") => parse_children(&child["children"], to_add),
                _ => {}
            }
        }
    }
}

impl SpyglassPlugin for Plugin {
    fn load(&mut self) {
        // Let the host know we want to check for updates on a regular interval.
        subscribe(PluginSubscription::CheckUpdateInterval);

        let mut path = None;

        // If the user has set the CHROME_DATA_FOLDER setting, use that
        if let Ok(folder) = std::env::var("CHROME_DATA_FOLDER") {
            if !folder.is_empty() {
                path = Some(Path::new(&folder).join(BOOKMARK_FILE))
            }
        }

        if path.is_none() {
            // Else detect the current HOST_OS and use the default folder
            // locations
            let host_os_res = std::env::var(consts::env::HOST_OS);
            let base_data_res = std::env::var(consts::env::BASE_DATA_DIR);
            let base_config_res = std::env::var(consts::env::BASE_CONFIG_DIR);

            if let (Ok(host_os), Ok(base_config_dir), Ok(base_data_dir)) =
                (host_os_res, base_config_res, base_data_res)
            {
                path = match host_os.as_str() {
                    // Linux is a little different and stores the bookmarks under ~/.config
                    // base_config_dir: /home/alice/.config
                    "linux" => Some(
                        Path::new(&base_config_dir)
                            .join("google-chrome/Default")
                            .join(BOOKMARK_FILE),
                    ),
                    // base_data_dir: /Users/alice/Library/Application Support
                    "macos" => Some(
                        Path::new(&base_data_dir)
                            .join("Google/Chrome/Default")
                            .join(BOOKMARK_FILE),
                    ),
                    // base_data_dir: C:\Users\Alice\AppData\Roaming
                    "windows" => Some(
                        Path::new(&base_data_dir)
                            .join("Google/Chrome/User Data/Default")
                            .join(BOOKMARK_FILE),
                    ),
                    _ => None,
                }
            }
        }

        // Grab bookmark file from chrome data folder, if available
        if let Some(path) = path {
            sync_file(DATA_DIR.to_string(), path.display().to_string());
        }
    }

    fn update(&mut self, _: PluginEvent) {
        let path = Path::new(DATA_DIR).join(BOOKMARK_FILE);
        // Nothing to do if theres no file.
        if !path.exists() {
            return;
        }

        match fs::read_to_string(path.clone()) {
            Ok(blob) => match self.parse_and_queue_bookmarks(&blob) {
                Ok(to_add) => enqueue_all(&to_add),
                Err(e) => log(format!("Unable to parse bookmark file: {e}")),
            },
            Err(e) => log(format!("Unable to read {}: {}", path.display(), e)),
        }
    }
}

impl Plugin {
    // Attempt to parse bookmark json
    pub fn parse_and_queue_bookmarks(&self, blob: &str) -> Result<Vec<String>, serde_json::Error> {
        let v: Value = serde_json::from_str(blob)?;
        let checksum_path = Path::new(DATA_DIR).join("checksum");

        // Previous checksum
        let previous_checksum = std::fs::read_to_string(checksum_path.clone()).ok();

        // Write out the checksum so we know when it was last checked
        let checksum = &v["checksum"];
        if let Some(checksum) = checksum.as_str() {
            let _ = std::fs::write(checksum_path, checksum);
            // If have a previous checksum saved, check it against the current one
            // and skip parsing bookmarks if they're the same.
            if let Some(previous_checksum) = previous_checksum {
                if previous_checksum == checksum {
                    return Ok(Vec::new());
                }
            }
        }

        // Return early if there is no root
        let root = &v["roots"];
        if !root.is_object() {
            return Ok(Vec::new());
        }

        let mut to_add: Vec<String> = Vec::new();

        // Parse the different bookmark types
        parse_children(&root["bookmark_bar"]["children"], &mut to_add);
        parse_children(&root["other"]["children"], &mut to_add);
        parse_children(&root["synced"]["children"], &mut to_add);

        Ok(to_add)
    }
}

#[cfg(test)]
mod test {
    use super::Plugin;

    #[test]
    fn test_parser() {
        let plugin = Plugin;
        let blob = include_str!("../../../fixtures/plugins/bookmarks.json");

        let res = plugin.parse_and_queue_bookmarks(blob);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), 3);
    }
}
