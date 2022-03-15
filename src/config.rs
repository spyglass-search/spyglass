use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Config {
    pub data_dir: PathBuf,
    pub prefs_dir: PathBuf,
    pub user_settings: UserSettings,
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UserSettings {
    /// Number of pages allowed per domain. Sub-domains are treated as
    /// separate domains.
    pub domain_crawl_limit: Limit,
    /// Should we run the setup wizard?
    pub run_wizard: bool,
}

impl Config {
    pub fn data_dir() -> PathBuf {
        let proj_dirs = ProjectDirs::from("com", "athlabs", "carto").unwrap();
        proj_dirs.data_dir().to_path_buf()
    }

    pub fn prefs_dir() -> PathBuf {
        let proj_dirs = ProjectDirs::from("com", "athlabs", "carto").unwrap();
        proj_dirs.preference_dir().to_path_buf()
    }

    /// User preferences file
    pub fn prefs_file() -> PathBuf {
        Self::prefs_dir().join("settings.ron")
    }

    pub fn new() -> Self {
        let data_dir = Config::data_dir();
        fs::create_dir_all(&data_dir).expect("Unable to create data folder");

        let prefs_dir = Config::prefs_dir();
        fs::create_dir_all(&prefs_dir).expect("Unable to create config folder");

        let prefs_path = Self::prefs_file();
        println!("Prefs path: {:?}", prefs_path);
        let user_settings = if prefs_path.exists() {
            ron::from_str(&fs::read_to_string(prefs_path).unwrap())
                .expect("Unable to read user preferences file.")
        } else {
            let settings = UserSettings::default();
            // Write out default settings
            fs::write(
                prefs_path,
                ron::ser::to_string_pretty(&settings, Default::default()).unwrap(),
            )
            .expect("Unable to save user preferences file.");
            settings
        };

        Config {
            data_dir,
            prefs_dir,
            user_settings,
        }
    }
}
