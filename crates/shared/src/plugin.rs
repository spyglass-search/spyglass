use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::UserSettings;
use crate::form::SettingOpts;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum PluginType {
    /// A more complex lens than a simple list of URLs
    /// - Registers itself as a lens, under some "trigger" label.
    /// - Enqueues URLs to the crawl queue.
    /// - Can register to handle specific protocols if not HTTP
    Lens,
}

pub type PluginUserSettings = HashMap<String, SettingOpts>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PluginConfig {
    pub name: String,
    pub author: String,
    pub description: String,
    pub version: String,
    // Trigger command for this plugin (if a lens),
    pub trigger: String,
    #[serde(default)]
    pub path: Option<PathBuf>,
    pub plugin_type: PluginType,
    pub user_settings: PluginUserSettings,
    #[serde(default)]
    pub is_enabled: bool,
}

impl PluginConfig {
    pub fn data_folder(&self) -> PathBuf {
        self.path
            .as_ref()
            .expect("Unable to find plugin path")
            .parent()
            .expect("Unable to find parent plugin directory")
            .join("data")
    }

    /// Update the plugin config based on user settings
    pub fn set_user_config(&mut self, user_settings: &UserSettings) {
        let plugin_user_settings = &user_settings.plugin_settings;
        if let Some(settings) = plugin_user_settings.get(&self.name) {
            // Loop through plugin settings and use any user overrides found.
            for (key, setting) in self.user_settings.iter_mut() {
                if let Some(value) = settings.get(key) {
                    setting.value = value.to_string();
                }
            }
        }
    }
}
