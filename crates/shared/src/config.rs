use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

pub const MAX_TOTAL_INFLIGHT: u32 = 100;
pub const MAX_DOMAIN_INFLIGHT: u32 = 100;

#[derive(Clone, Debug)]
pub struct Config {
    pub user_settings: UserSettings,
    pub lenses: HashMap<String, Lens>,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

/// Contexts are a set of domains/URLs/etc. that restricts a search space to
/// improve results.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Lens {
    #[serde(default = "Lens::default_author")]
    pub author: String,
    pub name: String,
    pub description: Option<String>,
    pub domains: Vec<String>,
    pub urls: Vec<String>,
    pub version: String,
}

impl Lens {
    fn default_author() -> String {
        "Unknown".to_string()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Limit {
    Infinite,
    Finite(u32),
}

impl Default for Limit {
    fn default() -> Self {
        Self::Finite(100)
    }
}

impl Limit {
    pub fn value(&self) -> u32 {
        match self {
            Limit::Infinite => u32::MAX,
            Limit::Finite(val) => *val,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserSettings {
    /// Number of pages allowed per domain. Sub-domains are treated as
    /// separate domains.
    pub domain_crawl_limit: Limit,
    /// Total number of in-flight crawls allowed for the entire app.
    pub inflight_crawl_limit: Limit,
    /// Number of in-flight crawls allowed per domain.
    pub inflight_domain_limit: Limit,
    /// Should we run the setup wizard?
    pub run_wizard: bool,
    /// Domains explicitly allowed, regardless of what's in the blocklist.
    pub allow_list: Vec<String>,
    /// Domains explicitly blocked from crawling.
    pub block_list: Vec<String>,
    #[serde(default = "UserSettings::default_shortcut")]
    pub shortcut: String,
}

impl UserSettings {
    fn default_shortcut() -> String {
        "CmdOrCtrl+Shift+/".to_string()
    }

    pub fn constraint_limits(&mut self) {
        // Make sure crawler limits are reasonable
        match self.inflight_crawl_limit {
            Limit::Infinite => self.inflight_crawl_limit = Limit::Finite(MAX_TOTAL_INFLIGHT),
            Limit::Finite(limit) => {
                self.inflight_crawl_limit = Limit::Finite(limit.min(MAX_TOTAL_INFLIGHT))
            }
        }

        match self.inflight_domain_limit {
            Limit::Infinite => self.inflight_domain_limit = Limit::Finite(MAX_DOMAIN_INFLIGHT),
            Limit::Finite(limit) => {
                self.inflight_domain_limit = Limit::Finite(limit.min(MAX_DOMAIN_INFLIGHT))
            }
        }
    }
}

impl Default for UserSettings {
    fn default() -> Self {
        UserSettings {
            domain_crawl_limit: Limit::Finite(10000),
            // 10 total crawlers at a time
            inflight_crawl_limit: Limit::Finite(10),
            // Limit to 2 crawlers for a domain
            inflight_domain_limit: Limit::Finite(2),
            // Not used at the moment
            run_wizard: false,
            allow_list: Vec::new(),
            block_list: vec!["web.archive.org".to_string()],
            shortcut: UserSettings::default_shortcut(),
        }
    }
}

impl Config {
    fn load_user_settings() -> anyhow::Result<UserSettings> {
        let prefs_path = Self::prefs_file();

        match prefs_path.exists() {
            true => {
                let mut settings: UserSettings =
                    ron::from_str(&fs::read_to_string(prefs_path).unwrap())?;
                settings.constraint_limits();
                Ok(settings)
            }
            _ => {
                let settings = UserSettings::default();
                // Write out default settings
                fs::write(
                    prefs_path,
                    ron::ser::to_string_pretty(&settings, Default::default()).unwrap(),
                )
                .expect("Unable to save user preferences file.");

                Ok(settings)
            }
        }
    }

    fn load_lenses() -> anyhow::Result<HashMap<String, Lens>> {
        let mut lenses = HashMap::new();

        let lense_dir = Self::lenses_dir();
        for entry in (fs::read_dir(lense_dir)?).flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().unwrap_or_default() == "ron" {
                if let Ok(file_contents) = fs::read_to_string(path) {
                    match ron::from_str::<Lens>(&file_contents) {
                        Err(err) => log::error!("Unable to load lens {:?}: {}", entry.path(), err),
                        Ok(lens) => {
                            log::info!("Loaded lens {}", lens.name);
                            lenses.insert(lens.name.clone(), lens);
                        }
                    }
                }
            }
        }

        if lenses.is_empty() {
            // Create a default lens as an example.
            let lens = Lens {
                author: "Spyglass".to_string(),
                version: "1".to_string(),
                name: "wiki".to_string(),
                description: Some(
                    "Search through official user-supported wikis for knowledge, games, and more."
                        .to_string(),
                ),
                domains: vec![
                    "en.wikipedia.org".to_string(),
                    "oldschool.runescape.wiki".to_string(),
                    "wiki.factorio.com".to_string(),
                ],
                urls: Vec::new(),
            };

            fs::write(
                Self::lenses_dir().join("wiki.ron"),
                ron::ser::to_string_pretty(&lens, Default::default()).unwrap(),
            )
            .expect("Unable to save default lens file.");
        }

        Ok(lenses)
    }

    pub fn app_identifier() -> String {
        if cfg!(debug_assertions) {
            "spyglass-dev".to_string()
        } else {
            "spyglass".to_string()
        }
    }

    pub fn data_dir() -> PathBuf {
        let proj_dirs = ProjectDirs::from("com", "athlabs", &Config::app_identifier()).unwrap();
        proj_dirs.data_dir().to_path_buf()
    }

    pub fn index_dir() -> PathBuf {
        Self::data_dir().join("index")
    }

    pub fn logs_dir() -> PathBuf {
        Self::data_dir().join("logs")
    }

    pub fn prefs_dir() -> PathBuf {
        let proj_dirs = ProjectDirs::from("com", "athlabs", &Config::app_identifier()).unwrap();
        log::info!("Using {:?}", proj_dirs.preference_dir().to_path_buf());
        proj_dirs.preference_dir().to_path_buf()
    }

    /// User preferences file
    pub fn prefs_file() -> PathBuf {
        Self::prefs_dir().join("settings.ron")
    }

    pub fn lenses_dir() -> PathBuf {
        Self::data_dir().join("lenses")
    }

    pub fn new() -> Self {
        let data_dir = Config::data_dir();
        fs::create_dir_all(&data_dir).expect("Unable to create data folder");

        let index_dir = Config::index_dir();
        fs::create_dir_all(&index_dir).expect("Unable to create index folder");

        let logs_dir = Config::logs_dir();
        fs::create_dir_all(&logs_dir).expect("Unable to create logs folder");

        let prefs_dir = Config::prefs_dir();
        fs::create_dir_all(&prefs_dir).expect("Unable to create config folder");

        let lenses_dir = Config::lenses_dir();
        fs::create_dir_all(&lenses_dir).expect("Unable to create `lenses` folder");

        // Gracefully handle issues loading user settings/lenses
        let user_settings = Self::load_user_settings().unwrap_or_else(|err| {
            log::warn!("Invalid user settings file! Reason: {}", err);
            Default::default()
        });

        let lenses = Self::load_lenses().unwrap_or_else(|err| {
            log::warn!("Unable to load lenses! Reason: {}", err);
            Default::default()
        });

        Config {
            lenses,
            user_settings,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::config::Config;

    #[test]
    #[ignore]
    pub fn test_load_lenses() {
        let res = Config::load_lenses();
        assert!(!res.is_err());
    }
}
