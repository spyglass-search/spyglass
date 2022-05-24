use std::fs;

use entities::models::{bootstrap_queue, lens};
use entities::regex::{regex_for_domain, regex_for_prefix, regex_for_robots};

use migration::sea_orm::DatabaseConnection;
use shared::config::{Config, Lens, LensRule, UserSettings};

use crate::crawler::bootstrap;
use crate::state::AppState;

pub struct LensRuleSets {
    allow_list: Vec<String>,
    skip_list: Vec<String>,
}

/// Create a set of allow/skip rules from a Lens
fn create_ruleset_from_lens(lens: &Lens) -> LensRuleSets {
    let mut allow_list = Vec::new();
    let mut skip_list: Vec<String> = Vec::new();

    // Build regex from domain
    for domain in lens.domains.iter() {
        allow_list.push(regex_for_domain(domain));
    }

    // Build regex from url rules
    for prefix in lens.urls.iter() {
        allow_list.push(regex_for_prefix(prefix));
    }

    // Build regex from rules
    for rule in lens.rules.iter() {
        match rule {
            LensRule::SkipURL(rule_str) => skip_list
                .push(regex_for_robots(&rule_str, entities::regex::WildcardType::Regex).unwrap()),
        }
    }

    LensRuleSets {
        allow_list,
        skip_list,
    }
}

async fn create_default_lens(config: &Config) {
    // Create a default lens as an example.
    let lens = Lens {
        author: "Spyglass".to_string(),
        version: "1".to_string(),
        name: "rust".to_string(),
        description: Some(
            "All things Rustlang. Search through Rust blogs, the rust book, and
            more."
                .to_string(),
        ),
        domains: vec!["blog.rust-lang.org".into()],
        urls: vec!["https://doc.rust-lang.org/book/".into()],
        is_enabled: true,
        rules: Vec::new(),
    };

    fs::write(
        config.lenses_dir().join("rust.ron"),
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

    // Bootstrap lenses.
    // Check & bootstrap will go through domains/prefixes and bootstrap a crawl queue
    // if we have not already done so.
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
    use super::{check_and_bootstrap, create_ruleset_from_lens};
    use entities::models::bootstrap_queue;
    use entities::test::setup_test_db;
    use regex::RegexSet;
    use shared::config::{Lens, UserSettings};

    #[tokio::test]
    async fn test_check_and_bootstrap() {
        let db = setup_test_db().await;
        let settings = UserSettings::default();
        let test = "https://example.com";

        bootstrap_queue::enqueue(&db, test, 10).await.unwrap();
        assert!(!check_and_bootstrap(&db, &settings, &test).await);
    }

    #[tokio::test]
    async fn test_create_ruleset() {
        let lens =
            ron::from_str::<Lens>(include_str!("../../../../fixtures/lens/test.ron")).unwrap();

        let rules = create_ruleset_from_lens(&lens);
        let allow_list = RegexSet::new(rules.allow_list).unwrap();
        let block_list = RegexSet::new(rules.skip_list).unwrap();

        let valid = "https://walkingdead.fandom.com/wiki/18_Miles_Out";
        let invalid = "https://walkingdead.fandom.com/wiki/Aaron_(Comic_Series)/Gallery";

        assert!(allow_list.is_match(valid));
        assert!(!block_list.is_match(valid));

        // Allowed without the SkipURL
        assert!(allow_list.is_match(invalid));
        // but should now be denied
        assert!(block_list.is_match(invalid));
    }
}
