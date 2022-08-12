use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::form::SettingOpts;

#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub enum PluginType {
    /// A more complex lens than a simple list of URLs
    /// - Registers itself as a lens, under some "trigger" label.
    /// - Enqueues URLs to the crawl queue.
    /// - Can register to handle specific protocols if not HTTP
    Lens,
}

pub type PluginUserSettings = HashMap<String, SettingOpts>;

#[derive(Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    pub name: String,
    pub author: String,
    pub description: String,
    pub version: String,
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
}
