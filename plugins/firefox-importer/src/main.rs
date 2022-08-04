use spyglass_plugin::*;
use std::path::{Path, PathBuf};

const DATA_DIR: &str = "/";
const DB_FILE: &str = "places.sqlite";
const BOOKMARK_QUERY: &str = "
    SELECT
        DISTINCT url
    FROM moz_bookmarks
    JOIN moz_places on moz_places.id = moz_bookmarks.fk
    WHERE
        moz_places.hidden = 0
        AND url like 'http%'
";

#[derive(Default)]
struct Plugin;

register_plugin!(Plugin);

impl SpyglassPlugin for Plugin {
    fn load(&mut self) {
        // Let the host know we want to check for updates on a regular interval.
        subscribe(PluginSubscription::CheckUpdateInterval);

        let mut profile_path = None;
        if let Ok(folder) = std::env::var("FIREFOX_DATA_FOLDER") {
            if !folder.is_empty() {
                profile_path = Some(Path::new(&folder).join(DB_FILE))
            }
        }

        if profile_path.is_none() {
            profile_path = self.default_profile_path();
        }

        // Grab a copy of the firefox data into our plugin data folder.
        // This is required because Firefox locks the file when running.
        if let Some(profile_path) = profile_path {
            sync_file(DATA_DIR.to_string(), profile_path.display().to_string());
        }
    }

    fn update(&mut self, _: PluginEvent) {
        let path = Path::new(DATA_DIR).join(DB_FILE);
        if path.exists() {
            enqueue_all(&self.read_bookmarks());
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
            if let Ok(entries) = list_dir(&profiles_dir.display().to_string(), false) {
                for path in entries {
                    if path.ends_with(".default") || path.ends_with(".default-release") {
                        return Some(Path::new(&path).join(DB_FILE));
                    }
                }
            }
        }

        None
    }

    fn read_bookmarks(&self) -> Vec<String> {
        let urls = sqlite3_query("places.sqlite", BOOKMARK_QUERY);
        if let Ok(urls) = urls {
            return urls;
        }

        Vec::new()
    }
}
