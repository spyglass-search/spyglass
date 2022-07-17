use spyglass_plugin::*;
use std::{fs, path::Path};

const DATA_DIR: &str = "/data";
const BOOKMARK_FILE: &str = "Bookmarks";

#[derive(Default)]
struct Plugin;

register_plugin!(Plugin);

impl SpyglassPlugin for Plugin {
    fn load(&self) {
        if let Ok(folder) = std::env::var("CHROME_DATA_FOLDER") {
            let path = Path::new(&folder).join(BOOKMARK_FILE);
            // Grab bookmark file from chrome data folder, if available
            sync_file(DATA_DIR.to_string(), path.display().to_string());
        }
    }

    fn request_queue(&self) {
        let path = Path::new(DATA_DIR).join(BOOKMARK_FILE);
        if path.exists() {
            match fs::read_to_string(path.clone()) {
                Ok(blob) => self.parse_and_queue_bookmarks(&blob),
                Err(e) => eprintln!("Unable to read {}: {}", path.display(), e),
            }
        }

        enqueue("https://docs.spyglass.fyi/install.html".into());
    }
}

impl Plugin {
    fn parse_and_queue_bookmarks(&self, blob: &String) {
        eprintln!("{}", blob);
    }
}
