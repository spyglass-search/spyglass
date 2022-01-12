use std::fs;

use directories::ProjectDirs;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Config {
    pub data_dir: PathBuf,
    pub prefs_dir: PathBuf,
}

impl Config {
    pub fn new() -> Self {
        let proj_dirs = ProjectDirs::from("com", "athlabs", "carto").unwrap();

        let data_dir = proj_dirs.data_dir();
        fs::create_dir_all(&data_dir).expect("Unable to create data folder");

        let prefs_dir = proj_dirs.preference_dir();
        fs::create_dir_all(&data_dir).expect("Unable to create config folder");

        Config {
            data_dir: data_dir.to_path_buf(),
            prefs_dir: prefs_dir.to_path_buf()
        }
    }
}