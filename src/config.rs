use std::fs;

use directories::ProjectDirs;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Config {
    pub data_dir: PathBuf,
    pub prefs_dir: PathBuf,
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

    pub fn new() -> Self {
        let data_dir = Config::data_dir();
        fs::create_dir_all(&data_dir).expect("Unable to create data folder");

        let prefs_dir = Config::prefs_dir();
        fs::create_dir_all(&data_dir).expect("Unable to create config folder");

        Config {
            data_dir,
            prefs_dir,
        }
    }
}
