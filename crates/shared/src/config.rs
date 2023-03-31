use crate::{
    form::{FormType, SettingOpts},
    plugin::PluginConfig,
};
use diff::Diff;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub use spyglass_lens::{
    types::{LensFilters, LensRule, LensSource},
    LensConfig, PipelineConfiguration,
};

mod audio;
mod filesystem;
mod user_actions;
pub use audio::*;
pub use filesystem::*;
pub use user_actions::*;

pub const MAX_TOTAL_INFLIGHT: u32 = 100;
pub const MAX_DOMAIN_INFLIGHT: u32 = 100;

// Name of legacy file importer plugin
pub const LEGACY_FILESYSTEM_PLUGIN: &str = "local-file-importer";
pub const LEGACY_PLUGIN_SETTINGS: &[&str] =
    &["local-file-importer", "chrome-importer", "firefox-importer"];

// Folder containing legacy local file importer plugin
pub const LEGACY_PLUGIN_FOLDERS: &[&str] =
    &["local-file-indexer", "chrome-importer", "firefox-importer"];

const USER_UUID_ENV_VAR: &str = "SPYGLASS_STATIC_USER_UUID";

// Represents a Tag on a document
pub type Tag = (String, String);

#[derive(Clone, Debug)]
pub struct Config {
    pub lenses: HashMap<String, LensConfig>,
    pub pipelines: HashMap<String, PipelineConfiguration>,
    pub user_settings: UserSettings,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Diff)]
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

// Enum of actions the user can take when a document is selected
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Diff)]
pub enum UserAction {
    OpenApplication(String, String),
    CopyToClipboard(String),
}

pub type PluginSettings = HashMap<String, HashMap<String, String>>;

#[derive(Clone, Debug, Deserialize, Serialize, Diff)]
pub struct UserSettings {
    /// Number of pages allowed per domain. Sub-domains are treated as
    /// separate domains.
    pub domain_crawl_limit: Limit,
    /// Total number of in-flight crawls allowed for the entire app.
    pub inflight_crawl_limit: Limit,
    /// Number of in-flight crawls allowed per domain.
    pub inflight_domain_limit: Limit,
    /// Have we run the wizard? false will run it again on startup.
    pub run_wizard: bool,
    /// Domains explicitly allowed, regardless of what's in the blocklist.
    pub allow_list: Vec<String>,
    /// Domains explicitly blocked from crawling.
    pub block_list: Vec<String>,
    /// Search bar activation hot key
    #[serde(default = "UserSettings::default_shortcut")]
    pub shortcut: String,
    /// Directory for metadata & index
    #[serde(default = "UserSettings::default_data_dir")]
    pub data_directory: PathBuf,
    /// Should we crawl links that don't match our lens rules?
    #[serde(default)]
    pub crawl_external_links: bool,
    /// Should we disable telemetry
    #[serde(default)]
    pub disable_telemetry: bool,
    #[serde(default)]
    pub filesystem_settings: FileSystemSettings,
    /// Plugin settings
    #[serde(default)]
    pub plugin_settings: PluginSettings,
    #[serde(default)]
    pub disable_autolaunch: bool,
    #[serde(default = "UserSettings::default_port")]
    pub port: u16,
    #[serde(default)]
    pub user_action_settings: UserActionSettings,
    #[serde(default)]
    pub audio_settings: AudioSettings,
    // /// Hide the app icon from the dock/taskbar while running. Will still show up
    // /// in the menubar/systemtray.
    // #[serde(default)]
    // pub hide_taskicon: bool,
}

impl UserSettings {
    pub fn default_data_dir() -> PathBuf {
        Config::default_data_dir()
    }

    pub fn default_shortcut() -> String {
        "CmdOrCtrl+Shift+/".to_string()
    }

    pub fn default_port() -> u16 {
        4664
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

// TODO: Turn this into procedural macro that we can use to tag attributes in the UserSetting struct
// NOTE: Settings page will be in the exact order that the config list is constructed.
impl From<UserSettings> for Vec<(String, SettingOpts)> {
    fn from(settings: UserSettings) -> Self {
        let mut config = vec![
            ("_.data_directory".into(), SettingOpts {
                label: "Data Directory".into(),
                value: settings.data_directory.to_str().map_or(String::new(), |s| s.to_string()),
                form_type: FormType::Path,
                restart_required: true,
                help_text: Some("The data directory is where your index, lenses, plugins, and logs are stored. This will require a restart.".into())
            }),
            ("_.disable_autolaunch".into(), SettingOpts {
                label: "Disable Autolaunch".into(),
                value: serde_json::to_string(&settings.disable_autolaunch).expect("Unable to ser autolaunch value"),
                form_type: FormType::Bool,
                restart_required: false,
                help_text: Some("Prevents Spyglass from automatically launching when your computer first starts up.".into())
            }),
            ("_.shortcut".into(), SettingOpts {
                label: "Global Shortcut".into(),
                value: settings.shortcut.clone(),
                form_type: FormType::KeyBinding,
                restart_required: false,
                help_text: Some("Defines the global keyboard shortcut used to open the Spyglass search window.".into())
            }),
            ("_.disable_telemetry".into(), SettingOpts {
                label: "Disable Telemetry".into(),
                value: serde_json::to_string(&settings.disable_telemetry).expect("Unable to ser autolaunch value"),
                form_type: FormType::Bool,
                restart_required: false,
                help_text: Some("Stop sending data to any 3rd-party service. See https://spyglass.fyi/telemetry for more info. This will require a restart.".into())
            }),
            ("_.port".into(), SettingOpts {
                label: "Spyglass Daemon Port".into(),
                value: settings.port.to_string(),
                form_type: FormType::Number,
                restart_required: true,
                help_text: Some("Port number used by the Spyglass background services. Only change this if you already have another server running on this port. This will require a restart.".into())
            }),
        ];

        if let Limit::Finite(val) = settings.inflight_crawl_limit {
            config.push((
                "_.inflight_crawl_limit".into(),
                SettingOpts {
                    label: "Max number of crawlers".into(),
                    value: val.to_string(),
                    form_type: FormType::Number,
                    restart_required: true,
                    help_text: Some(
                        "Maximum number of concurrent crawlers in total used by Spyglass, This will require a restart".into(),
                    ),
                },
            ));
        }

        if let Limit::Finite(val) = settings.inflight_domain_limit {
            config.push((
                "_.inflight_domain_limit".into(),
                SettingOpts {
                    label: "Max number crawlers per domain".into(),
                    value: val.to_string(),
                    form_type: FormType::Number,
                    restart_required: false,
                    help_text: Some(
                        "Maximum number of concurrent crawlers used per site/app.".into(),
                    ),
                },
            ));
        }

        config.extend(fs_setting_opts(&settings));
        config.extend(audio_setting_opts(&settings));

        config
    }
}

impl Default for UserSettings {
    fn default() -> Self {
        UserSettings {
            // Max number of pages to crawl per domain
            domain_crawl_limit: Limit::Finite(500000),
            // 10 total crawlers at a time
            inflight_crawl_limit: Limit::Finite(10),
            // Limit to 2 crawlers for a domain
            inflight_domain_limit: Limit::Finite(2),
            run_wizard: false,
            allow_list: Vec::new(),
            block_list: vec!["web.archive.org".to_string()],
            // Activation shortcut
            shortcut: UserSettings::default_shortcut(),
            // Where to store the metadata & index
            data_directory: UserSettings::default_data_dir(),
            crawl_external_links: false,
            disable_telemetry: false,
            filesystem_settings: FileSystemSettings::default(),
            plugin_settings: Default::default(),
            disable_autolaunch: false,
            port: UserSettings::default_port(),
            user_action_settings: UserActionSettings::default(),
            audio_settings: AudioSettings::default(),
        }
    }
}

impl Config {
    pub fn save_user_settings(user_settings: &UserSettings) -> anyhow::Result<()> {
        let prefs_path = Self::prefs_file();
        let serialized = ron::ser::to_string_pretty(user_settings, Default::default())
            .expect("Unable to serialize user settings");
        fs::write(prefs_path, serialized).expect("Unable to save user preferences file");

        Ok(())
    }

    pub fn load_user_settings() -> anyhow::Result<UserSettings> {
        let prefs_path = Self::prefs_file();

        match prefs_path.exists() {
            true => {
                let contents = &fs::read_to_string(prefs_path).unwrap_or_default();
                let mut settings: UserSettings = ron::from_str(contents)?;
                settings.constraint_limits();
                Ok(settings)
            }
            _ => {
                let settings = UserSettings::default();
                // Write out default settings
                fs::write(
                    prefs_path,
                    ron::ser::to_string_pretty(&settings, Default::default())
                        .expect("Unable to serialize settings."),
                )
                .expect("Unable to save user preferences file.");

                Ok(settings)
            }
        }
    }

    pub fn migrate_user_settings(mut settings: UserSettings) -> anyhow::Result<UserSettings> {
        // convert local-filesystem-config to user settings filesystem config
        let mut modified: bool = false;
        for setting in LEGACY_PLUGIN_SETTINGS {
            let res = settings.plugin_settings.remove(&setting.to_string());
            if setting == &"local-file-importer" {
                if let Some(local_file_settings) = res {
                    modified = true;

                    let dir_list = local_file_settings
                        .get("FOLDERS_LIST")
                        .map(|f| f.to_owned())
                        .unwrap_or_default();

                    if let Ok(dirs) = serde_json::from_str::<HashSet<String>>(&dir_list) {
                        settings.filesystem_settings.watched_paths =
                            dirs.iter().map(PathBuf::from).collect::<Vec<PathBuf>>();
                    }

                    let ext_list = local_file_settings
                        .get("EXTS_LIST")
                        .map(|s| s.to_owned())
                        .unwrap_or_default();
                    if let Ok(exts) = serde_json::from_str::<HashSet<String>>(&ext_list) {
                        settings.filesystem_settings.supported_extensions =
                            exts.iter().cloned().collect();
                    }
                }
            }
        }

        if modified {
            let _ = Self::save_user_settings(&settings);
            return Self::load_user_settings();
        }

        Ok(settings)
    }

    /// Load & read plugin manifests to get plugin settings that are available.
    pub fn load_plugin_config(&self) -> HashMap<String, PluginConfig> {
        let plugins_dir = self.plugins_dir();
        let plugin_files = fs::read_dir(plugins_dir).expect("Invalid plugin directory");
        let mut settings: HashMap<String, PluginConfig> = Default::default();

        for entry in plugin_files.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Load plugin settings
            let plugin_config = path.join("manifest.ron");
            if !plugin_config.exists() || !plugin_config.is_file() {
                log::warn!("Invalid plugin manifest: {}", path.as_path().display());
                continue;
            }

            if let Ok(file_contents) = std::fs::read_to_string(plugin_config) {
                match ron::from_str::<PluginConfig>(&file_contents) {
                    Ok(plugin_config) => {
                        let mut config = plugin_config.clone();
                        config.path = Some(path.join("main.wasm"));

                        settings.insert(plugin_config.name.clone(), config.clone());
                    }
                    Err(error) => log::error!("Error loading plugin config {:?}", error),
                }
            }
        }

        settings
    }

    fn cleanup_legacy_plugins(plugin_dir: &Path) {
        for folder in LEGACY_PLUGIN_FOLDERS {
            let fs_plugin_path = plugin_dir.join(folder);
            if fs_plugin_path.exists() {
                if let Err(err) = fs::remove_dir_all(fs_plugin_path) {
                    log::warn!("Error removing plugin {folder} - {:?}", err);
                }
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

    pub fn machine_identifier() -> String {
        let uid_file = Self::prefs_dir().join("uid");
        if uid_file.exists() {
            std::fs::read_to_string(uid_file).unwrap_or_default()
        } else {
            let new_uid = match std::env::var(USER_UUID_ENV_VAR) {
                Ok(uid) => uid,
                Err(_) => {
                    // Generate a random ID and associate it with this machine for error/metrics.
                    Uuid::new_v4().as_hyphenated().to_string()
                }
            };
            let _ = std::fs::write(uid_file, new_uid.clone());
            new_uid
        }
    }

    pub fn default_data_dir() -> PathBuf {
        let proj_dirs = ProjectDirs::from("com", "athlabs", &Config::app_identifier())
            .expect("Unable to find a default data directory");
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

    pub fn model_dir(&self) -> PathBuf {
        self.data_dir().join("models")
    }

    pub fn prefs_dir() -> PathBuf {
        let proj_dirs = ProjectDirs::from("com", "athlabs", &Config::app_identifier())
            .expect("Unable to find a suitable settings directory");
        proj_dirs.preference_dir().to_path_buf()
    }

    /// User preferences file
    pub fn prefs_file() -> PathBuf {
        Self::prefs_dir().join("settings.ron")
    }

    pub fn plugins_dir(&self) -> PathBuf {
        self.data_dir().join("plugins")
    }

    pub fn lenses_dir(&self) -> PathBuf {
        self.data_dir().join("lenses")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir().join("cache")
    }

    pub fn pipelines_dir(&self) -> PathBuf {
        self.data_dir().join("pipelines")
    }

    pub fn new() -> Self {
        let prefs_dir = Config::prefs_dir();
        fs::create_dir_all(prefs_dir).expect("Unable to create config folder");

        // Gracefully handle issues loading user settings/lenses
        let user_settings = Self::load_user_settings().unwrap_or_else(|err| {
            log::error!("Invalid user settings file! Reason: {}", err);
            Default::default()
        });

        let user_settings = Self::migrate_user_settings(user_settings).unwrap_or_else(|err| {
            log::error!("Invalid user settings file! Reason: {}", err);
            Default::default()
        });

        let config = Config {
            lenses: HashMap::new(),
            pipelines: HashMap::new(),
            user_settings,
        };

        fs::create_dir_all(config.data_dir()).expect("Unable to create data folder");
        fs::create_dir_all(config.index_dir()).expect("Unable to create index folder");
        fs::create_dir_all(config.logs_dir()).expect("Unable to create logs folder");
        fs::create_dir_all(config.lenses_dir()).expect("Unable to create `lenses` folder");
        fs::create_dir_all(config.pipelines_dir()).expect("Unable to create `pipelines` folder");
        fs::create_dir_all(config.plugins_dir()).expect("Unable to create `plugin` folder");
        fs::create_dir_all(config.model_dir()).expect("Unable to create models folder");

        Self::cleanup_legacy_plugins(&config.plugins_dir());

        config
    }
}
