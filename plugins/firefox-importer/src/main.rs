use spyglass_plugin::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DATA_DIR: &str = "/";
const DB_FILE: &str = "places.sqlite";
// How often we want to sync w/ the firefox database
const SYNC_INTERVAL_S: u64 = 60 * 5;
// SQL query to find bookmarks
const BOOKMARK_QUERY: &str = "SELECT DISTINCT url FROM moz_bookmarks JOIN moz_places on moz_places.id = moz_bookmarks.fk WHERE moz_places.hidden = 0 AND url like 'http%'";

struct Plugin {
    last_update: Instant,
    profile_path: Option<PathBuf>,
}

impl Default for Plugin {
    fn default() -> Self {
        Plugin {
            last_update: Instant::now(),
            profile_path: None,
        }
    }
}

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
            self.profile_path = Some(profile_path.clone());
            log(format!("Using profile: {}", profile_path.display()));
            sync_file(DATA_DIR.to_string(), profile_path.display().to_string());
        }
    }

    fn update(&mut self, _: PluginEvent) {
        let path = Path::new(DATA_DIR).join(DB_FILE);

        // Perioodically resync w/ Firefox database
        if let Some(profile_path) = &self.profile_path {
            if self.last_update.elapsed() >= Duration::from_secs(SYNC_INTERVAL_S) {
                self.last_update = Instant::now();
                sync_file(DATA_DIR.to_string(), profile_path.display().to_string())
            }
        }

        if path.exists() {
            self.read_bookmarks();
        } else {
            log(format!(
                "Unable to find places.sqlite file @ {}",
                path.to_string_lossy()
            ));
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
            (host_os_res.clone(), host_home_res, base_data_res)
        {
            // Determined from https://support.mozilla.org/en-US/kb/profiles-where-firefox-stores-user-data
            match host_os.as_str() {
                "linux" => Some(Path::new(&home_dir).join(".mozilla/firefox")),
                "macos" => {
                    Some(Path::new(&home_dir).join("Library/Application Support/Firefox/Profiles"))
                }
                "windows" => Some(
                    Path::new(&format!("{}\\Mozilla\\Firefox\\Profile\\", &data_dir)).to_path_buf(),
                ),
                _ => None,
            }
        } else {
            None
        };

        // Loop through profiles in the profile directory & find the default one.
        // A little hacky since Firefox prepends a random string to the profile name.
        if let (Ok(host_os), Some(profiles_dir)) = (host_os_res, profiles_dir) {
            if let Ok(entries) = list_dir(&profiles_dir.display().to_string()) {
                for entry in entries {
                    if entry.is_dir
                        && (entry.path.ends_with(".default")
                            || entry.path.ends_with(".default-release"))
                    {
                        return match host_os.as_str() {
                            "windows" => Some(
                                Path::new(&format!("{}\\{}", &entry.path, DB_FILE)).to_path_buf(),
                            ),
                            _ => Some(Path::new(&entry.path).join(DB_FILE)),
                        };
                    }
                }
            }
        }

        None
    }

    fn read_bookmarks(&self) {
        sqlite3_query("places.sqlite", BOOKMARK_QUERY);
    }
}
