use serde_json::Value;
use spyglass_plugin::*;
use std::{fs, path::Path};

const DATA_DIR: &str = "/data";
const BOOKMARK_FILE: &str = "Bookmarks";

#[derive(Default)]
struct Plugin;

register_plugin!(Plugin);

impl SpyglassPlugin for Plugin {
    fn load(&self) {
        let path = {
            // If the user has set the CHROME_DATA_FOLDER setting, use that
            if let Ok(folder) = std::env::var("CHROME_DATA_FOLDER") {
                Some(Path::new(&folder).join(BOOKMARK_FILE))
            } else {
                // Else detect the current HOST_OS and use the default folder
                // locations
                let host_os_res = std::env::var(consts::env::HOST_OS);
                let base_data_res = std::env::var(consts::env::BASE_DATA_DIR);
                let base_config_res = std::env::var(consts::env::BASE_CONFIG_DIR);

                if let (Ok(host_os), Ok(base_config_dir), Ok(base_data_dir)) =
                    (host_os_res, base_config_res, base_data_res)
                {
                    match host_os.as_str() {
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
                } else {
                    None
                }
            }
        };

        // Grab bookmark file from chrome data folder, if available
        if let Some(path) = path {
            sync_file(DATA_DIR.to_string(), path.display().to_string());
        }
    }

    fn update(&self) {
        let path = Path::new(DATA_DIR).join(BOOKMARK_FILE);
        // Nothing to do if theres no file.
        if !path.exists() {
            return;
        }

        match fs::read_to_string(path.clone()) {
            Ok(blob) => {
                if let Err(e) = self.parse_and_queue_bookmarks(&blob) {
                    log(format!("Unable to parse bookmark file: {}", e));
                }
            }
            Err(e) => eprintln!("Unable to read {}: {}", path.display(), e),
        }
    }
}

impl Plugin {
    fn parse_children(&self, children: &Value, to_add: &mut Vec<String>) {
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
                    Some("folder") => self.parse_children(&child["children"], to_add),
                    _ => {}
                }
            }
        }
    }

    // Attempt to parse bookmark json
    pub fn parse_and_queue_bookmarks(&self, blob: &str) -> Result<usize, serde_json::Error> {
        let v: Value = serde_json::from_str(blob)?;

        // Write out the checksum so we know when it was last checked
        let checksum = &v["checksum"];
        if let Some(checksum) = checksum.as_str() {
            let checksum_file = Path::new(DATA_DIR).join("checksum");
            let _ = std::fs::write(checksum_file, checksum);
        }

        // Return early if there is no root
        let root = &v["roots"];
        if !root.is_object() {
            return Ok(0);
        }

        let mut to_add: Vec<String> = Vec::new();

        // Parse the different bookmark types
        self.parse_children(&root["bookmark_bar"]["children"], &mut to_add);
        self.parse_children(&root["other"]["children"], &mut to_add);
        self.parse_children(&root["synced"]["children"], &mut to_add);

        // Add URLs to queue.
        enqueue_all(&to_add);

        Ok(to_add.len())
    }
}

#[cfg(test)]
mod test {
    use super::Plugin;

    #[test]
    fn test_parser() {
        let plugin = Plugin;
        let blob = include_str!("../../../fixtures/plugins/bookmarks.json");

        let res = plugin.parse_and_queue_bookmarks(&blob.to_string());
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), 3);
    }
}
