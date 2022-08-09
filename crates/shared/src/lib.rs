use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

pub mod config;
pub mod event;
pub mod plugin;
pub mod regex;
pub mod request;
pub mod response;
pub mod rpc;

#[derive(Clone, Debug, Display, EnumString, PartialEq, Serialize, Deserialize)]
pub enum FormType {
    List,
    Text,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SettingOpts {
    pub label: String,
    pub value: String,
    pub form_type: FormType,
    pub help_text: Option<String>,
}

pub fn url_path_windows(path: &str) -> String {
    // Unescape colons & spaces
    let mut url_path = path.replace("%3A", ":").replace("%20", " ");
    // Strip superfluous path prefix
    url_path = url_path
        .strip_prefix('/')
        .map(|s| s.to_string())
        .unwrap_or(url_path);
    // Convert path dividers into Windows specific ones.
    url_path = url_path.replace('/', "\\");

    url_path
}
