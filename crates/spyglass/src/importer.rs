use dirs::home_dir;
use shared::config::Config;
use sqlx::sqlite::SqlitePoolOptions;
use std::{env, fs, path::PathBuf};

use entities::models::crawl_queue;
use libspyglass::state::AppState;
use shared::config::Lens;

#[allow(dead_code)]
pub struct FirefoxImporter {
    pub profile_path: Option<PathBuf>,
    pub imported_path: PathBuf,
    pub config: Config,
}

impl FirefoxImporter {
    /// Get the default profile path for Firefox
    #[allow(dead_code)]
    fn default_profile_path() -> Result<PathBuf, &'static str> {
        let home = home_dir().expect("No home directory detected");
        match env::consts::OS {
            "linux" => Ok(home.join(".mozilla/firefox")),
            "macos" => Ok(home.join("Library/Application Support/Firefox/Profiles")),
            // "windows" => {},
            _ => Err("Platform not supported"),
        }
    }

    #[allow(dead_code)]
    pub fn new(config: &Config) -> Self {
        let mut profile_path = None;

        // Detect Firefox profiles
        if let Ok(path) = FirefoxImporter::default_profile_path() {
            profile_path = Some(path);
        }

        let imported_path = config.data_dir().join("firefox.sqlite");
        FirefoxImporter {
            profile_path,
            imported_path,
            config: config.clone(),
        }
    }

    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
        let to_add: Vec<String> = rows.into_iter().map(|(_, x)| x).collect();
        // import firefox with its own lens to allow domains.
        let firefox_lens = Lens {
            author: "SpyGlass".into(),
            name: "Firefox".into(),
            description: Some("Firefox history dump".to_string()),
            domains: vec!["*".to_string()],
            urls: vec![],
            version: "1".into(),
            is_enabled: true,
            rules: vec![],
        };
        let lenses: Vec<Lens> = vec![firefox_lens];
        match crawl_queue::enqueue_all(
            &state.db,
            &to_add,
            &lenses,
            &self.config.user_settings,
            &Default::default(),
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                log::error!("Importer firefox crawl_queue {}", e);
            }
        }
        count += to_add.len();

        log::info!("imported {} urls", count);

        Ok(())
    }

    #[allow(dead_code)]
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
    use crate::importer::FirefoxImporter;
    use shared::config::Config;

    #[test]
    #[ignore]
    fn test_detect_profiles() {
        let config = Config::new();
        let importer = FirefoxImporter::new(&config);
        let profiles = importer.detect_profiles();
        assert!(profiles.len() > 0);
    }
}
