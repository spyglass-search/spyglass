pub use crate::keyboard::{KeyCode, ModifiersState};
use directories::{ProjectDirs, UserDirs};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub use crate::accelerator::{self, Accelerator};
use crate::response::SearchResult;
use crate::{
    form::{FormType, SettingOpts},
    plugin::PluginConfig,
};
pub use spyglass_lens::types::{LensRule, LensSource};
pub use spyglass_lens::{LensConfig, PipelineConfiguration};

pub const MAX_TOTAL_INFLIGHT: u32 = 100;
pub const MAX_DOMAIN_INFLIGHT: u32 = 100;

// Name of legacy file importer plugin
pub const LEGACY_FILESYSTEM_PLUGIN: &str = "local-file-importer";
pub const LEGACY_PLUGIN_SETTINGS: &[&str] =
    &["local-file-importer", "chrome-importer", "firefox-importer"];

// Folder containing legacy local file importer plugin
pub const LEGACY_PLUGIN_FOLDERS: &[&str] =
    &["local-file-indexer", "chrome-importer", "firefox-importer"];

// The default extensions
pub const DEFAULT_EXTENSIONS: &[&str] = &["docx", "html", "md", "txt", "ods", "xls", "xlsx"];

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

pub type PluginSettings = HashMap<String, HashMap<String, String>>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FileSystemSettings {
    #[serde(default)]
    pub enable_filesystem_scanning: bool,
    #[serde(default = "FileSystemSettings::default_paths")]
    pub watched_paths: Vec<PathBuf>,
    #[serde(default = "FileSystemSettings::default_extensions")]
    pub supported_extensions: Vec<String>,
}

impl FileSystemSettings {
    pub fn default_paths() -> Vec<PathBuf> {
        let mut file_paths: Vec<PathBuf> = Vec::new();

        if let Some(user_dirs) = UserDirs::new() {
            if let Some(path) = user_dirs.desktop_dir() {
                file_paths.push(path.to_path_buf());
            }

            if let Some(path) = user_dirs.document_dir() {
                file_paths.push(path.to_path_buf());
            }
        }
        file_paths
    }

    pub fn default_extensions() -> Vec<String> {
        DEFAULT_EXTENSIONS
            .iter()
            .map(|val| String::from(*val))
            .collect()
    }
}

// Represents a Tag on a document
pub type Tag = (String, String);

impl Default for FileSystemSettings {
    fn default() -> Self {
        FileSystemSettings {
            enable_filesystem_scanning: false,
            watched_paths: FileSystemSettings::default_paths(),
            supported_extensions: FileSystemSettings::default_extensions(),
        }
    }
}

// Enum of actions the user can take when a document is selected
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum UserAction {
    OpenApplication(String, String),
    CopyToClipboard(String),
}

// The user action settings configuration provides the ability
// for the user to define custom behavior for a document.
#[derive(Clone, Debug, Deserialize, Serialize, Default, PartialEq)]
pub struct UserActionSettings {
    pub actions: Vec<UserActionDefinition>,
    pub context_actions: Vec<ContextActions>,
}

impl UserActionSettings {
    // Helper used to identify if the user action settings contains
    // an action that would be triggered by the passed in keyboard
    // combination.
    pub fn contains_trigger(
        &self,
        modifiers: &ModifiersState,
        key: &KeyCode,
        context: Option<&SearchResult>,
        os: &str,
    ) -> bool {
        for action in &self.actions {
            if action.is_triggered(modifiers, key, os) {
                return true;
            }
        }

        if let Some(context) = context {
            for action in &self.context_actions {
                if action.is_applicable(context) && action.contains_trigger(modifiers, key, os) {
                    return true;
                }
            }
        }
        false
    }

    // Helper method used to access the action definition for the action
    // that should be triggered by the passed in keyboard combination.
    // Note that context specific actions will be checked before general
    // actions. This allows a user to configure a general action for all
    // documents and custom actions for specific documents
    pub fn get_triggered_action(
        &self,
        modifiers: &ModifiersState,
        key: &KeyCode,
        os: &str,
        context: Option<&SearchResult>,
    ) -> Option<UserActionDefinition> {
        if let Some(context) = context {
            for action in &self.context_actions {
                if action.is_applicable(context) {
                    if let Some(context_action) = action.get_triggered_action(modifiers, key, os) {
                        return Some(context_action);
                    }
                }
            }
        }

        for action in &self.actions {
            if action.is_triggered(modifiers, key, os) {
                return Some(action.clone());
            }
        }

        None
    }

    // List of default actions when no other actions are configured
    pub fn default_actions() -> UserActionSettings {
        UserActionSettings {
            actions: vec![UserActionDefinition {
                action: UserAction::CopyToClipboard(String::from("{{ url }}")),
                key_binding: String::from("CmdOrCtrl+C"),
                label: String::from("Copy URL to Clipboard"),
                status_msg: Some(String::from("Copying...")),
            }],
            context_actions: vec![],
        }
    }
}

// Defines context specific actions. A context specific action
// is a list of actions that are only valid when the document selected
// matches the defined context.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ContextActions {
    // Defines what context must be matched for the actions to be valid
    pub context: ContextFilter,
    // The list of actions for this context
    pub actions: Vec<UserActionDefinition>,
}

impl ContextActions {
    // Helper method used to identify if the context contains an actions that should
    // be triggered by the passed in keyboard combination. Note this method does not
    // check to see if the context is valid, just if the key combination matches.
    pub fn contains_trigger(&self, modifiers: &ModifiersState, key: &KeyCode, os: &str) -> bool {
        for action in &self.actions {
            if action.is_triggered(modifiers, key, os) {
                return true;
            }
        }
        false
    }

    // Helper method used to access the action that should be triggered by the passed
    // in key combination. Note this method does not check to see if the context is valid.
    pub fn get_triggered_action(
        &self,
        modifiers: &ModifiersState,
        key: &KeyCode,
        os: &str,
    ) -> Option<UserActionDefinition> {
        for action in &self.actions {
            if action.is_triggered(modifiers, key, os) {
                return Some(action.clone());
            }
        }
        None
    }

    // Helper method used to identify if the context actions are valid based on the
    // passed in search result
    pub fn is_applicable(&self, context: &SearchResult) -> bool {
        let mut current_tags = HashMap::new();
        for (tag, value) in &context.tags {
            current_tags
                .entry(tag.clone())
                .or_insert(Vec::new())
                .push(value.clone());
        }
        // Process exclude tag first to remove unwanted items
        if let Some(exclude_types) = &self.context.exclude_tag_type {
            for tag_type in exclude_types {
                if current_tags.contains_key(tag_type.as_str()) {
                    return false;
                }
            }
        }

        if let Some(exclude_tags) = &self.context.exclude_tag {
            for (tag, value) in exclude_tags {
                if let Some(current_vals) = current_tags.get(tag.as_str()) {
                    if current_vals.contains(value) {
                        return false;
                    }
                }
            }
        }

        let tags_configured = self.context.has_tag_type.is_some() | self.context.has_tag.is_some();
        let mut include: bool = !tags_configured;
        if let Some(tags) = &self.context.has_tag_type {
            for tag in tags {
                if current_tags.contains_key(tag.as_str()) {
                    // The current context has a tag type we are
                    // looking for, set to true and break out of
                    // the loop
                    include |= true;
                    break;
                }
            }
        }

        if !include {
            if let Some(tags) = &self.context.has_tag {
                for (tag, value) in tags {
                    if let Some(current_vals) = current_tags.get(tag.as_str()) {
                        if current_vals.contains(value) {
                            // The current context has a tag we are
                            // looking for, set to true and break out of
                            // the loop
                            include |= true;
                            break;
                        }
                    }
                }
            }
        }

        if let Some(urls) = &self.context.url_like {
            let mut has_url = false;
            for url in urls {
                if url.eq(&context.url) {
                    has_url = true;
                    break;
                }
            }
            // We have urls to check so the context must match
            // one of the urls and any tags that were
            // previously defined
            include &= has_url;
        }
        include
    }
}

// Filter definition used to define what documents should match
// against the context.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ContextFilter {
    // Includes documents that match any of the defined tags
    pub has_tag: Option<Vec<Tag>>,
    // Includes documents that match any of the defined tag types
    pub has_tag_type: Option<Vec<String>>,
    // Excludes documents that match the specified tag
    pub exclude_tag: Option<Vec<Tag>>,
    // Exclude documents that match the defined tag type
    pub exclude_tag_type: Option<Vec<String>>,
    // Include only documents that match the specified url. When
    // set a document must match the url and any specified tags
    // to be included
    pub url_like: Option<Vec<String>>,
}

// The definition for an action
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct UserActionDefinition {
    pub label: String,
    pub status_msg: Option<String>,
    pub action: UserAction,
    pub key_binding: String,
}

impl UserActionDefinition {
    // Helper used to identify if the key binding specified in the action definition
    // matches the passed in key code and modifiers
    pub fn is_triggered(&self, modifiers: &ModifiersState, key: &KeyCode, os: &str) -> bool {
        //todo preprocess accelerator like a normal person
        if let Ok(accelerator) = accelerator::parse_accelerator(&self.key_binding, os) {
            return accelerator.matches(modifiers, key);
        }
        false
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
    /// Have we run the wizard? false will run it again on startup.
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
    #[serde(default = "UserActionSettings::default_actions")]
    pub user_action_settings: UserActionSettings,
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
impl From<UserSettings> for Vec<(String, SettingOpts)> {
    fn from(settings: UserSettings) -> Self {
        let mut config = vec![
            ("_.data_directory".into(), SettingOpts {
                label: "Data Directory".into(),
                value: settings.data_directory.to_str().map_or(String::new(), |s| s.to_string()),
                form_type: FormType::Path,
                help_text: Some("The data directory is where your index, lenses, plugins, and logs are stored. This will require a restart.".into())
            }),
            ("_.disable_autolaunch".into(), SettingOpts {
                label: "Disable Autolaunch".into(),
                value: serde_json::to_string(&settings.disable_autolaunch).expect("Unable to ser autolaunch value"),
                form_type: FormType::Bool,
                help_text: Some("Prevents Spyglass from automatically launching when your computer first starts up.".into())
            }),
            ("_.disable_telemetry".into(), SettingOpts {
                label: "Disable Telemetry".into(),
                value: serde_json::to_string(&settings.disable_telemetry).expect("Unable to ser autolaunch value"),
                form_type: FormType::Bool,
                help_text: Some("Stop sending data to any 3rd-party service. See https://spyglass.fyi/telemetry for more info.".into())
            }),
            ("_.port".into(), SettingOpts {
                label: "Spyglass Daemon Port".into(),
                value: settings.port.to_string(),
                form_type: FormType::Number,
                help_text: Some("Port number used by the Spyglass background services. Only change this if you already have another serive running on this port.".into())
            }),
            ("_.filesystem_settings.enable_filesystem_scanning".into(), SettingOpts {
                label: "Enable Filesystem Indexing".into(),
                value: settings.filesystem_settings.enable_filesystem_scanning.to_string(),
                form_type: FormType::Bool,
                help_text: Some("Enables and disables local filesystem indexing. When enabled configured folders will be scanned and indexed. Any supported file types will have their contents indexed.".into())
            }),
            ("_.filesystem_settings.watched_paths".into(), SettingOpts {
                label: "Folder List".into(),
                value: serde_json::to_string(&settings.filesystem_settings.watched_paths).unwrap_or(String::from("[]")),
                form_type: FormType::PathList,
                help_text: Some("List of folders that will be crawled & indexed. These folders will be crawled recursively, so you only need to specifiy the parent folder.".into())
            }),
            ("_.filesystem_settings.supported_extensions".into(), SettingOpts {
                label: "Extension List".into(),
                value: serde_json::to_string(&settings.filesystem_settings.supported_extensions).unwrap_or(String::from("[]")),
                form_type: FormType::StringList,
                help_text: Some("List of file types to index.".into())
            }),
        ];

        if let Limit::Finite(val) = settings.inflight_crawl_limit {
            config.push((
                "_.inflight_crawl_limit".into(),
                SettingOpts {
                    label: "Max number of crawlers".into(),
                    value: val.to_string(),
                    form_type: FormType::Number,
                    help_text: Some(
                        "Maximum number of concurrent crawlers in total used by Spyglass".into(),
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
                    help_text: Some(
                        "Maximum number of concurrent crawlers used per site/app.".into(),
                    ),
                },
            ));
        }

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
                if let Ok(plugin_config) = ron::from_str::<PluginConfig>(&file_contents) {
                    let mut config = plugin_config.clone();
                    config.path = Some(path.join("main.wasm"));

                    settings.insert(plugin_config.name.clone(), config.clone());
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
            // Generate a random ID and associate it with this machine for error/metrics.
            let new_uid = Uuid::new_v4().as_hyphenated().to_string();
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

        let data_dir = config.data_dir();
        fs::create_dir_all(data_dir).expect("Unable to create data folder");

        let index_dir = config.index_dir();
        fs::create_dir_all(index_dir).expect("Unable to create index folder");

        let logs_dir = config.logs_dir();
        fs::create_dir_all(logs_dir).expect("Unable to create logs folder");

        let lenses_dir = config.lenses_dir();
        fs::create_dir_all(lenses_dir).expect("Unable to create `lenses` folder");

        let pipelines_dir = config.pipelines_dir();
        fs::create_dir_all(pipelines_dir).expect("Unable to create `pipelines` folder");

        let plugins_dir = config.plugins_dir();
        fs::create_dir_all(&plugins_dir).expect("Unable to create `plugin` folder");

        Self::cleanup_legacy_plugins(&plugins_dir);

        config
    }
}
