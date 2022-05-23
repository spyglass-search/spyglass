use std::fs;

use entities::models::{bootstrap_queue, lens};
use migration::sea_orm::DatabaseConnection;
use shared::config::{Config, Lens, UserSettings};

use crate::crawler::bootstrap;
use crate::state::AppState;

async fn create_default_lens(config: &Config) {
    // Create a default lens as an example.
    let lens = Lens {
        author: "Spyglass".to_string(),
        version: "1".to_string(),
        name: "wiki".to_string(),
        description: Some(
            "Search through official user-supported wikis for knowledge, games, and more."
                .to_string(),
        ),
        domains: vec!["blog.rust-lang.org".into(), "wiki.factorio.com".into()],
        urls: vec![
            "https://https://en.wikipedia.org/wiki/Portal:".into(),
            "https://doc.rust-lang.org/book/".into(),
            "https://oldschool.runescape.wiki/w/".into(),
        ],
        is_enabled: true,
    };

    fs::write(
        config.lenses_dir().join("wiki.ron"),
        ron::ser::to_string_pretty(&lens, Default::default()).unwrap(),
    )
    .expect("Unable to save default lens file.");
}

/// Read lenses into the AppState
pub async fn read_lenses(state: &AppState, config: &Config) -> anyhow::Result<()> {
    state.lenses.clear();

    let lense_dir = config.lenses_dir();
    let mut num_lenses = 0;

    for entry in (fs::read_dir(lense_dir)?).flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().unwrap_or_default() == "ron" {
            if let Ok(file_contents) = fs::read_to_string(path) {
                match ron::from_str::<Lens>(&file_contents) {
                    Err(err) => log::error!("Unable to load lens {:?}: {}", entry.path(), err),
                    Ok(lens) => {
                        num_lenses += 1;
                        if lens.is_enabled {
                            state.lenses.insert(lens.name.clone(), lens);
                        }
                    }
                }
            }
        }
    }

    if num_lenses == 0 {
        create_default_lens(config).await;
    }

    Ok(())
}

/// Loop through lenses in the AppState. Update our internal db & bootstrap anything
/// that hasn't been bootstrapped.
pub async fn load_lenses(state: AppState) {
    let _ = lens::reset(&state.db).await;

    let mut new_lenses: Vec<Lens> = Vec::new();
    for entry in state.lenses.iter() {
        let lens = entry.value();
        // Have we added this lens to the database?
        match lens::add(
            &state.db,
            &lens.name,
            &lens.author,
            lens.description.as_ref(),
            &lens.version,
        )
        .await
        {
            Ok(true) => {
                log::info!("loaded lens {}", lens.name);
                new_lenses.push(lens.clone());
            }
            Ok(false) => log::info!("duplicate lens ({})", lens.name),
            Err(e) => log::error!("error loading lens {}", e),
        }
    }

    // Bootstrap new lenses
    for lens in new_lenses {
        for domain in lens.domains.iter() {
            let seed_url = format!("https://{}", domain);
            check_and_bootstrap(&state.db, &state.user_settings, &seed_url).await;
        }

        for prefix in lens.urls.iter() {
            check_and_bootstrap(&state.db, &state.user_settings, prefix).await;
        }
    }
}

/// Check if we've already bootstrapped a prefix / otherwise add it to the queue.
async fn check_and_bootstrap(
    db: &DatabaseConnection,
    user_settings: &UserSettings,
    seed_url: &str,
) -> bool {
    if let Ok(false) = bootstrap_queue::has_seed_url(db, seed_url).await {
        log::info!("bootstrapping {}", seed_url);

        match bootstrap::bootstrap(db, user_settings, seed_url).await {
            Err(e) => {
                log::error!("{}", e);
                return false;
            }
            Ok(cnt) => {
                log::info!("bootstrapped {} w/ {} urls", seed_url, cnt);
                let _ = bootstrap_queue::enqueue(db, seed_url, cnt as i64).await;
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod test {
    use super::check_and_bootstrap;
    use entities::models::bootstrap_queue;
    use entities::test::setup_test_db;
    use shared::config::UserSettings;

    #[tokio::test]
    async fn test_check_and_bootstrap() {
        let db = setup_test_db().await;
        let settings = UserSettings::default();
        let test = "https://example.com";

        bootstrap_queue::enqueue(&db, test, 10).await.unwrap();
        assert!(!check_and_bootstrap(&db, &settings, &test).await);
    }
}
