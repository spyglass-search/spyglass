use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct Config {
    pub user_settings: UserSettings,
    pub lenses: HashMap<String, Lense>,
}

/// Contexts are a set of domains/URLs/etc. that restricts a search space to
/// improve results.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Lense {
    pub name: String,
    pub domains: Vec<String>,
    pub urls: Vec<String>,
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
    /// Domains explicitly allowed, regardless of what's in the blocklist.
    pub allow_list: Vec<String>,
    /// Domains explicitly blocked from crawling.
    pub block_list: Vec<String>,
}

impl Config {
    fn _load_user_settings() -> UserSettings {
        let prefs_path = Self::prefs_file();
        if prefs_path.exists() {
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
        }
    }

    fn _load_lenses() -> anyhow::Result<HashMap<String, Lense>> {
        let mut lenses = HashMap::new();

        let lense_dir = Self::lenses_dir();
        for entry in (fs::read_dir(lense_dir)?).flatten() {
            let path = entry.path();
            if path.is_file() {
                match ron::from_str::<Lense>(&fs::read_to_string(path).unwrap()) {
                    Err(err) => log::error!("Unable to load lense {:?}: {}", entry.path(), err),
                    Ok(lense) => {
                        log::info!("Loaded lense {}", lense.name);
                        lenses.insert(lense.name.clone(), lense);
                    }
                }
            }
        }

        Ok(lenses)
    }

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

    pub fn lenses_dir() -> PathBuf {
        Self::data_dir().join("lenses")
    }

    pub fn new() -> Self {
        let data_dir = Config::data_dir();
        fs::create_dir_all(&data_dir).expect("Unable to create data folder");

        let prefs_dir = Config::prefs_dir();
        fs::create_dir_all(&prefs_dir).expect("Unable to create config folder");

        let lenses_dir = Config::lenses_dir();
        fs::create_dir_all(&lenses_dir).expect("Unable to create `lenses` folder");

        Config {
            lenses: Self::_load_lenses().expect("Unable to load lenses"),
            user_settings: Self::_load_user_settings(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::config::Config;

    #[test]
    pub fn test_load_lenses() {
        let res = Config::_load_lenses();
        assert!(!res.is_err());
    }
}
