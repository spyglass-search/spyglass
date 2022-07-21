use spyglass_plugin::*;
use std::path::{Path, PathBuf};

const DATA_DIR: &str = "/data";
const DB_FILE: &str = "places.sqlite";

#[derive(Default)]
struct Plugin;

register_plugin!(Plugin);

impl SpyglassPlugin for Plugin {
    fn load(&self) {
        // Let the host know we want to check for updates on a regular interval.
        subscribe(PluginEvent::CheckUpdateInterval);

        // Grab a copy of the firefox data into our plugin data folder.
        // This is required because Firefox locks the file when running.
        if let Some(profile_path) = self.default_profile_path() {
            sync_file(DATA_DIR.to_string(), profile_path.display().to_string());
        }
    }

    fn update(&self) {
        let path = Path::new(DATA_DIR).join(DB_FILE);
        if path.exists() {
            self.read_bookmarks();
        }
    }
}

impl Plugin {
    /// Detect the default profile based on the OS
    fn default_profile_path(&self) -> Option<PathBuf> {
        let host_os_res = std::env::var(consts::env::HOST_OS);
        let host_home_res = std::env::var(consts::env::HOST_HOME_DIR);
        let base_data_res = std::env::var(consts::env::BASE_DATA_DIR);

        let profiles_dir = if let (Ok(host_os), Ok(home_dir), Ok(data_dir)) =
            (host_os_res, host_home_res, base_data_res)
        {
            // Determined from https://support.mozilla.org/en-US/kb/profiles-where-firefox-stores-user-data
            match host_os.as_str() {
                "linux" => Some(Path::new(&home_dir).join(".mozilla/firefox")),
                "macos" => {
                    Some(Path::new(&home_dir).join("Library/Application Support/Firefox/Profiles"))
                }
                "windows" => Some(Path::new(&data_dir).join("Mozilla/Firefox/Profile/")),
                _ => None,
            }
        } else {
            None
        };

        // Loop through profiles in the profile directory & find the default one.
        // A little hacky since Firefox prepends a random string to the profile name.
        if let Some(profiles_dir) = profiles_dir {
            if let Ok(entries) = list_dir(&profiles_dir.display().to_string()) {
                for path in entries {
                    if path.ends_with(".default") || path.ends_with(".default-release") {
                        return Some(Path::new(&path).join(DB_FILE));
                    }
                }
            }
        }

        None
    }

    fn read_bookmarks(&self) {
        let urls = sqlite3_query("places.sqlite", "SELECT url FROM moz_places LIMIT 10");
        if let Ok(urls) = urls {
            for url in urls {
                eprintln!("{}", url);
            }
        }
    }
}