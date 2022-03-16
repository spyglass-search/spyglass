use dirs::home_dir;
use sqlx::sqlite::SqlitePoolOptions;
use std::{env, fs, path::PathBuf};

use crate::config::Config;
use crate::models::crawl_queue;
use crate::state::AppState;

pub struct FirefoxImporter {
    pub profile_path: Option<PathBuf>,
    pub imported_path: PathBuf,
    pub config: Config,
}

impl FirefoxImporter {
    /// Get the default profile path for Firefox
    fn default_profile_path() -> Result<PathBuf, &'static str> {
        let home = home_dir().expect("No home directory detected");
        match env::consts::OS {
            // "linux" => {},
            "macos" => Ok(home.join("Library/Application Support/Firefox/Profiles")),
            // "windows" => {},
            _ => Err("Platform not supported"),
        }
    }

    pub fn new(config: &Config) -> Self {
        let mut profile_path = None;

        // Detect Firefox profiles
        if let Ok(path) = FirefoxImporter::default_profile_path() {
            profile_path = Some(path);
        }

        let imported_path = config.data_dir.join("firefox.sqlite");
        FirefoxImporter {
            profile_path,
            imported_path,
            config: config.clone(),
        }
    }

    pub fn detect_profiles(&self) -> Vec<PathBuf> {
        let mut path_results = Vec::new();
        if let Some(path) = &self.profile_path {
            for path in fs::read_dir(path).unwrap().flatten() {
                if path.path().is_dir() {
                    let db_path = path.path().join("places.sqlite");
                    if db_path.exists() {
                        path_results.push(db_path);
                    }
                }
            }
        }

        path_results
    }

    /// Add Firefox history into our crawl queue.
    async fn copy_history(&self, state: &AppState) -> anyhow::Result<()> {
        log::info!("Importing history");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!(
                "sqlite://{}",
                self.imported_path.to_str().unwrap()
            ))
            .await?;

        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, url
                FROM moz_places
                WHERE hidden = 0
                ORDER BY visit_count DESC",
        )
        .fetch_all(&pool)
        .await?;

        let mut count = 0;
        for (_, url) in rows.iter() {
            crawl_queue::enqueue(&state.db, url, &self.config.user_settings).await?;
            count += 1;
        }

        log::info!("imported {} urls", count);

        Ok(())
    }

    pub async fn import(&self, state: &AppState) -> anyhow::Result<PathBuf> {
        let profiles = self.detect_profiles();
        let path = profiles.first().expect("No Firefox history detected");

        // TODO: Check when the file was last updated and copy if newer.
        if !self.imported_path.exists() {
            fs::copy(path, &self.imported_path)?;
            self.copy_history(state).await?;
        }

        Ok(self.imported_path.clone())
    }
}

#[cfg(test)]
mod test {
    use crate::config::Config;
    use crate::importer::FirefoxImporter;

    #[test]
    fn test_detect_profiles() {
        let config = Config::new();
        let importer = FirefoxImporter::new(&config);
        let profiles = importer.detect_profiles();
        assert!(profiles.len() > 0);
    }
}
