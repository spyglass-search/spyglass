use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::regex::{regex_for_robots, WildcardType};

pub const MAX_TOTAL_INFLIGHT: u32 = 100;
pub const MAX_DOMAIN_INFLIGHT: u32 = 100;

pub type PluginSettings = HashMap<String, HashMap<String, String>>;
#[derive(Clone, Debug)]
pub struct Config {
    pub user_settings: UserSettings,
    pub plugin_settings: PluginSettings,
    pub lenses: HashMap<String, Lens>,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

/// Different rules that filter out the URLs that would be crawled for a lens
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum LensRule {
    /// Robots.txt regex to skip certain URLs
    /// Skips are applied when bootstrapping & crawling
    SkipURL(String),
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
    #[serde(default = "Lens::default_is_enabled")]
    pub is_enabled: bool,
    #[serde(default)]
    pub rules: Vec<LensRule>,
}

impl Lens {
    fn default_author() -> String {
        "Unknown".to_string()
    }

    fn default_is_enabled() -> bool {
        true
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
    /// Search bar activation hot key
    #[serde(default = "UserSettings::default_shortcut")]
    /// Directory for metadata & index
    pub shortcut: String,
    #[serde(default = "UserSettings::default_data_dir")]
    pub data_directory: PathBuf,
    /// Should we crawl links that don't match our lens rules?
    #[serde(default)]
    pub crawl_external_links: bool,
    /// Should we disable telemetry
    #[serde(default)]
    pub disable_telementry: bool,
}

impl UserSettings {
    fn default_data_dir() -> PathBuf {
        Config::default_data_dir()
    }

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

impl From<UserSettings> for HashMap<String, String> {
    fn from(settings: UserSettings) -> Self {
        let mut map: HashMap<String, String> = HashMap::new();
        map.insert(
            "user.data_directory".to_string(),
            settings
                .data_directory
                .to_str()
                .expect("Unable to convert to string")
                .to_string(),
        );

        map
    }
}

impl Default for UserSettings {
    fn default() -> Self {
        UserSettings {
            domain_crawl_limit: Limit::Finite(500000),
            // 10 total crawlers at a time
            inflight_crawl_limit: Limit::Finite(10),
            // Limit to 2 crawlers for a domain
            inflight_domain_limit: Limit::Finite(2),
            // Not used at the moment
            run_wizard: false,
            allow_list: Vec::new(),
            block_list: vec!["web.archive.org".to_string()],
            // Activation shortcut
            shortcut: UserSettings::default_shortcut(),
            // Where to store the metadata & index
            data_directory: UserSettings::default_data_dir(),
            crawl_external_links: false,
            disable_telementry: false,
        }
    }
}

impl Config {
    pub fn save_plugin_settings(&self, settings: &PluginSettings) -> anyhow::Result<()> {
        let prefs_path = self.plugin_settings_file();
        fs::write(
            prefs_path,
            ron::ser::to_string_pretty(settings, Default::default())?,
        )
        .expect("Unable to save plugin settings file.");

        Ok(())
    }

    pub fn save_user_settings(&self, user_settings: &UserSettings) -> anyhow::Result<()> {
        let prefs_path = Self::prefs_file();
        let serialized = ron::ser::to_string_pretty(user_settings, Default::default())
            .expect("Unable to serialize user settings");
        fs::write(prefs_path, serialized).expect("Unable to save user preferences file");

        Ok(())
    }

    fn load_plugin_setings(&self) -> anyhow::Result<PluginSettings> {
        let prefs_path = self.plugin_settings_file();
        if prefs_path.exists() {
            let settings: PluginSettings = ron::from_str(&fs::read_to_string(prefs_path)?)?;
            return Ok(settings);
        }

        // Create default settings
        let settings: PluginSettings = Default::default();
        self.save_plugin_settings(&settings)?;

        Ok(settings)
    }

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

    pub fn app_identifier() -> String {
        if cfg!(debug_assertions) {
            "spyglass-dev".to_string()
        } else {
            "spyglass".to_string()
        }
    }

    pub fn default_data_dir() -> PathBuf {
        let proj_dirs = ProjectDirs::from("com", "athlabs", &Config::app_identifier()).unwrap();
        proj_dirs.data_dir().to_path_buf()
    }

    pub fn data_dir(&self) -> PathBuf {
        if self.user_settings.data_directory != Self::default_data_dir() {
            self.user_settings.data_directory.clone()
        } else {
            Self::default_data_dir()
        }
    }

    pub fn index_dir(&self) -> PathBuf {
        self.data_dir().join("index")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir().join("logs")
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

    pub fn plugins_dir(&self) -> PathBuf {
        self.data_dir().join("plugins")
    }

    pub fn plugin_settings_file(&self) -> PathBuf {
        self.plugins_dir().join("settings.ron")
    }

    pub fn lenses_dir(&self) -> PathBuf {
        self.data_dir().join("lenses")
    }

    pub fn new() -> Self {
        let prefs_dir = Config::prefs_dir();
        fs::create_dir_all(&prefs_dir).expect("Unable to create config folder");

        // Gracefully handle issues loading user settings/lenses
        let user_settings = Self::load_user_settings().unwrap_or_else(|err| {
            log::error!("Invalid user settings file! Reason: {}", err);
            Default::default()
        });

        let mut config = Config {
            lenses: HashMap::new(),
            user_settings,
            plugin_settings: Default::default(),
        };

        let data_dir = config.data_dir();
        fs::create_dir_all(&data_dir).expect("Unable to create data folder");

        let index_dir = config.index_dir();
        fs::create_dir_all(&index_dir).expect("Unable to create index folder");

        let logs_dir = config.logs_dir();
        fs::create_dir_all(&logs_dir).expect("Unable to create logs folder");

        let lenses_dir = config.lenses_dir();
        fs::create_dir_all(&lenses_dir).expect("Unable to create `lenses` folder");

        let plugins_dir = config.plugins_dir();
        fs::create_dir_all(&plugins_dir).expect("Unable to create `plugin` folder");

        // Load plugin settings
        let plugin_settings = config.load_plugin_setings().unwrap_or_else(|err| {
            log::error!("Invalid plugin settings file!: Reason: {}", err);
            Default::default()
        });
        config.plugin_settings = plugin_settings;

        config
    }
}
