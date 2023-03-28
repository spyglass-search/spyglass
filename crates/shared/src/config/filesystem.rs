use diff::Diff;
use directories::UserDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::form::{FormType, SettingOpts};

use super::UserSettings;

pub fn fs_setting_opts(settings: &UserSettings) -> Vec<(String, SettingOpts)> {
    vec![
        ("_.filesystem_settings.enable_filesystem_scanning".into(), SettingOpts {
            label: "Enable Filesystem Indexing".into(),
            value: settings.filesystem_settings.enable_filesystem_scanning.to_string(),
            form_type: FormType::Bool,
            restart_required: false,
            help_text: Some("Enables and disables local filesystem indexing. When enabled configured folders will be scanned and indexed. Any supported file types will have their contents indexed.".into())
        }),
        ("_.filesystem_settings.watched_paths".into(), SettingOpts {
            label: "Folder List".into(),
            value: serde_json::to_string(&settings.filesystem_settings.watched_paths).unwrap_or(String::from("[]")),
            form_type: FormType::PathList,
            restart_required: false,
            help_text: Some("List of folders that will be crawled & indexed. These folders will be crawled recursively, so you only need to specify the parent folder.".into())
        }),
        ("_.filesystem_settings.supported_extensions".into(), SettingOpts {
            label: "Extension List".into(),
            value: serde_json::to_string(&settings.filesystem_settings.supported_extensions).unwrap_or(String::from("[]")),
            form_type: FormType::StringList,
            restart_required: false,
            help_text: Some("List of file types to index.".into())
        }),
    ]
}

// The default extensions. This are in addition to the ones we already support
pub const DEFAULT_EXTENSIONS: &[&str] = &["docx", "html", "md", "txt", "ods", "xls", "xlsx"];

#[derive(Clone, Debug, Deserialize, Serialize, Diff)]
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

impl Default for FileSystemSettings {
    fn default() -> Self {
        FileSystemSettings {
            enable_filesystem_scanning: false,
            watched_paths: FileSystemSettings::default_paths(),
            supported_extensions: FileSystemSettings::default_extensions(),
        }
    }
}
