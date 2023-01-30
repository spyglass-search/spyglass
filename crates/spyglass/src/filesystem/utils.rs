extern crate glob;
use chrono::{DateTime, NaiveDateTime, Utc};
use ignore::{gitignore::Gitignore, Error};
use std::{
    collections::HashSet,
    ffi::OsStr,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use url::Url;

use crate::state::AppState;

// Create a file URI
pub fn path_to_uri(path: &Path) -> String {
    path_string_to_uri(path.display().to_string())
}

// // Create a file URI
// pub fn path_buf_to_uri(path: &PathBuf) -> String {
//     path_string_to_uri(path.display().to_string())
// }

pub fn path_string_to_uri(path_str: String) -> String {
    // Eventually this will be away to keep track of multiple devices and searching across
    // them.
    let host = "";

    let mut new_url = Url::parse("file://").expect("Base URI");
    let _ = new_url.set_host(Some(host));
    // Fixes issues handling windows drive letters
    let path_str = path_str.replace(':', "%3A");
    // Fixes an issue where DirEntry adds too many escapes.
    let path_str = path_str.replace(r#"\\\\"#, r#"\"#);
    let path_str = path_str.replace(r#"\\"#, r#"\"#);

    new_url.set_path(&path_str);
    new_url.to_string()
}

/// Converts a uri to a valid path buf
pub fn uri_to_path(uri: &str) -> anyhow::Result<PathBuf> {
    match Url::parse(uri) {
        Ok(url) => match url.to_file_path() {
            Ok(path) => Ok(path),
            Err(_) => Err(anyhow::format_err!("Unable to access file path")),
        },
        Err(error) => Err(anyhow::Error::from(error)),
    }
}

/// Identifies if the provided path represents a windows shortcut
pub fn is_windows_shortcut(path: &Path) -> bool {
    let ext = &path
        .extension()
        .and_then(|x| x.to_str())
        .map(|x| x.to_string())
        .unwrap_or_default();
    ext.eq("lnk")
}

/// Helper method used to get the destination for a windows shortcut. Note that
/// this method currently only checks the local base path and local base path unicode
pub fn get_shortcut_destination(path: &Path) -> Option<PathBuf> {
    let shortcut = lnk::ShellLink::open(path);
    if let Ok(shortcut) = shortcut {
        if let Some(link_info) = &shortcut.link_info() {
            if link_info.local_base_path().is_some() {
                return Some(PathBuf::from(link_info.local_base_path().clone().unwrap()));
            } else if link_info.local_base_path_unicode().is_some() {
                return Some(PathBuf::from(
                    link_info.local_base_path_unicode().clone().unwrap(),
                ));
            }
        }
    }
    None
}

/// Accessor for the last modified time for a file. If the last modified
/// time is not available now is returned
pub fn last_modified_time_for_path(path: &Path) -> DateTime<Utc> {
    if let Ok(metadata) = path.metadata() {
        if let Ok(modified) = metadata.modified() {
            let since_the_epoch = modified
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards");

            if let Some(time) =
                NaiveDateTime::from_timestamp_millis(since_the_epoch.as_millis() as i64)
            {
                DateTime::<Utc>::from_utc(time, Utc)
            } else {
                Utc::now()
            }
        } else {
            Utc::now()
        }
    } else {
        Utc::now()
    }
}

/// Accessor for the last modified time for a file. If the last modified
/// time is not available now is returned
pub fn last_modified_time(path: &Path) -> DateTime<Utc> {
    if let Ok(metadata) = path.metadata() {
        if let Ok(modified) = metadata.modified() {
            let since_the_epoch = modified
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards");

            if let Some(time) =
                NaiveDateTime::from_timestamp_millis(since_the_epoch.as_millis() as i64)
            {
                DateTime::<Utc>::from_utc(time, Utc)
            } else {
                Utc::now()
            }
        } else {
            Utc::now()
        }
    } else {
        Utc::now()
    }
}

/// Helper method used to access the configured file search directories from
/// user settings.
pub fn get_search_directories(state: &AppState) -> Vec<PathBuf> {
    let plugin_settings = state.user_settings.plugin_settings.clone();
    if let Some(local_file_settings) = plugin_settings.get("local-file-importer") {
        let dir_list = local_file_settings.get("FOLDERS_LIST");

        let directories =
            if let Ok(dirs) = serde_json::from_str::<HashSet<String>>(dir_list.unwrap().as_str()) {
                dirs
            } else {
                HashSet::new()
            };

        directories
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<PathBuf>>()
    } else {
        Vec::new()
    }
}

/// Helper method used to access the configured file extensions from
/// user settings.
pub fn get_supported_file_extensions(state: &AppState) -> HashSet<String> {
    let plugin_settings = state.user_settings.plugin_settings.clone();

    if let Some(local_file_settings) = plugin_settings.get("local-file-importer") {
        let ext_list = local_file_settings.get("EXTS_LIST");
        if let Ok(exts) = serde_json::from_str::<HashSet<String>>(ext_list.unwrap().as_str()) {
            return exts;
        }
    }
    HashSet::new()
}

/// Helper method used to identify if the provided path represents a gitignore file
pub fn is_ignore_file(path: &Path) -> bool {
    if let Some(file_name) = path.file_name() {
        return file_name.eq(OsStr::new(".gitignore"));
    }
    false
}

/// Helper method used to convert a gitignore file into a processed gitignore object
pub fn patterns_from_file(path: &Path) -> Result<Gitignore, Error> {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(path.parent().unwrap());
    builder.add(path);
    builder.build()
}

/// Helper method used to identify if the provided path is in a "hidden" directory.
/// In this case a "hidden" directory is any directory that starts with "." Example:
/// .git
/// .ssh
pub fn is_in_hidden_dir(path: &Path) -> bool {
    path.ancestors().any(|ancestor| {
        if ancestor.is_dir() {
            if let Some(name) = ancestor.file_name() {
                if let Some(name_str) = name.to_str() {
                    if name_str.starts_with('.') {
                        return true;
                    }
                }
            }
        }
        false
    })
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use super::path_to_uri;
    use url::Url;

    #[test]
    fn test_path_to_uri() {
        #[cfg(target_os = "windows")]
        let test_folder = Path::new("C:\\tmp\\path_to_uri");

        #[cfg(not(target_os = "windows"))]
        let test_folder = Path::new("/tmp/path_to_uri");

        std::fs::create_dir_all(test_folder).expect("Unable to create test dir");

        let test_path = test_folder.join("test.txt");
        let uri = path_to_uri(&test_path.to_path_buf());

        #[cfg(target_os = "windows")]
        assert_eq!(uri, "file://localhost/C%3A/tmp/path_to_uri/test.txt");
        #[cfg(not(target_os = "windows"))]
        assert_eq!(uri, "file://localhost/tmp/path_to_uri/test.txt");

        let url = Url::parse(&uri).unwrap();
        let file_path = url.to_file_path().unwrap();
        assert_eq!(file_path, test_path);

        if test_folder.exists() {
            std::fs::remove_dir_all(test_folder).expect("Unable to clean up test folder");
        }
    }
}
