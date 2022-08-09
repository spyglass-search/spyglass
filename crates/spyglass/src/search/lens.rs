use std::fs;

use entities::models::crawl_queue::EnqueueSettings;
use entities::models::{bootstrap_queue, crawl_queue, indexed_document, lens};
use entities::sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use migration::sea_orm::DatabaseConnection;
use shared::regex::{regex_for_robots, WildcardType};
use url::Url;

use shared::config::{Config, Lens, LensRule, UserSettings};

use crate::crawler::bootstrap;
use crate::search::Searcher;
use crate::state::AppState;

/// Check if we've already bootstrapped a prefix / otherwise add it to the queue.
async fn check_and_bootstrap(
    lens: &Lens,
    db: &DatabaseConnection,
    user_settings: &UserSettings,
    seed_url: &str,
) -> bool {
    if let Ok(false) = bootstrap_queue::has_seed_url(db, seed_url).await {
        log::info!("bootstrapping {}", seed_url);

        match bootstrap::bootstrap(lens, db, user_settings, seed_url).await {
            Err(e) => {
                log::error!("bootstrap {}", e);
                return false;
            }
            Ok(cnt) => {
                log::info!("bootstrapped {} w/ {} urls", seed_url, cnt);
                let _ = bootstrap_queue::enqueue(db, seed_url, cnt as i64).await;
                return true;
            }
        }
    } else {
        log::info!(
            "bootstrap queue already contains seed url: {}, skipping",
            seed_url
        );
    }

    false
}

/// Read lenses into the AppState
pub async fn read_lenses(state: &AppState, config: &Config) -> anyhow::Result<()> {
    state.lenses.clear();

    let lense_dir = config.lenses_dir();

    for entry in (fs::read_dir(lense_dir)?).flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().unwrap_or_default() == "ron" {
            if let Ok(file_contents) = fs::read_to_string(path) {
                match ron::from_str::<Lens>(&file_contents) {
                    Err(err) => log::error!("Unable to load lens {:?}: {}", entry.path(), err),
                    Ok(lens) => {
                        if lens.is_enabled {
                            state.lenses.insert(lens.name.clone(), lens);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Loop through lenses in the AppState. Update our internal db & bootstrap anything
/// that hasn't been bootstrapped.
pub async fn load_lenses(state: AppState) {
    let mut new_lenses: Vec<Lens> = Vec::new();
    for entry in state.lenses.iter() {
        let lens = entry.value();
        // Have we added this lens to the database?
        match lens::add_or_enable(
            &state.db,
            &lens.name,
            &lens.author,
            lens.description.as_ref(),
            &lens.version,
            lens::LensType::Simple,
        )
        .await
        {
            Ok(is_new) => {
                log::info!("loaded lens {}, new? {}", lens.name, is_new);
                new_lenses.push(lens.clone());
            }
            Err(e) => log::error!("error loading lens {}", e),
        }
    }

    // Bootstrap lenses.
    // Check & bootstrap will go through domains/prefixes and bootstrap a crawl queue
    // if we have not already done so.
    for lens in new_lenses {
        for domain in lens.domains.iter() {
            let seed_url = format!("https://{}", domain);
            check_and_bootstrap(&lens, &state.db, &state.user_settings, &seed_url).await;
        }

        for prefix in lens.urls.iter() {
            // Handle singular URL matches
            if prefix.ends_with('$') {
                // Remove the '$' suffix and add to the crawl queue
                let url = prefix.strip_suffix('$').expect("No $ at end of prefix");
                if let Err(err) = crawl_queue::enqueue_all(
                    &state.db,
                    &[url.to_owned()],
                    &[],
                    &state.user_settings,
                    &EnqueueSettings {
                        force_allow: true,
                        ..Default::default()
                    },
                )
                .await
                {
                    log::warn!("unable to enqueue <{}> due to {}", prefix, err)
                }
            } else {
                check_and_bootstrap(&lens, &state.db, &state.user_settings, prefix).await;
            }
        }

        // Rules will go through and remove crawl tasks AND indexed_documents that match.
        for rule in lens.rules.iter() {
            match rule {
                LensRule::SkipURL(rule_str) => {
                    if let Some(rule_like) = regex_for_robots(rule_str, WildcardType::Database) {
                        // Remove matching crawl tasks
                        let _ = crawl_queue::remove_by_rule(&state.db, &rule_like).await;
                        // Remove matching indexed documents
                        match indexed_document::remove_by_rule(&state.db, &rule_like).await {
                            Ok(doc_ids) => {
                                if let Ok(mut writer) = state.index.writer.lock() {
                                    for doc_id in doc_ids {
                                        let res = Searcher::delete(&mut writer, &doc_id);
                                        if let Err(err) = res {
                                            log::error!("Unable to remove docs: {:?}", err);
                                        }
                                    }
                                }
                            }
                            Err(e) => log::error!("Unable to remove docs: {:?}", e),
                        }
                    }
                }
                LensRule::LimitURLDepth(rule_str, _) => {
                    // Remove URLs that don't match this rule
                    // sqlite3 does support regexp, but this is _not_ guaranteed to
                    // be on all platforms, so we'll apply this in a brute-force way.
                    if let Ok(parsed) = Url::parse(rule_str) {
                        if let Some(domain) = parsed.host_str() {
                            // Remove none matchin URLs from crawl_queue
                            let urls = crawl_queue::Entity::find()
                                .filter(crawl_queue::Column::Domain.eq(domain))
                                .all(&state.db)
                                .await;

                            let regex = regex::Regex::new(&rule.to_regex())
                                .expect("Invalid LimitURLDepth regex");

                            let mut num_removed = 0;
                            if let Ok(urls) = urls {
                                for crawl in urls {
                                    if !regex.is_match(&crawl.url) {
                                        num_removed += 1;
                                        let _ = crawl.delete(&state.db).await;
                                    }
                                }
                            }
                            log::info!("removed {} docs from crawl_queue", num_removed);

                            // Remove none matchin URLs from indexed documents
                            let mut num_removed = 0;
                            let indexed = indexed_document::Entity::find()
                                .filter(indexed_document::Column::Domain.eq(domain))
                                .all(&state.db)
                                .await;

                            let mut doc_ids = Vec::new();
                            if let Ok(indexed) = indexed {
                                for doc in indexed {
                                    if !regex.is_match(&doc.url) {
                                        num_removed += 1;
                                        doc_ids.push(doc.doc_id.clone());
                                        let _ = doc.delete(&state.db).await;
                                    }
                                }
                            }

                            if let Ok(mut writer) = state.index.writer.lock() {
                                for doc_id in doc_ids {
                                    let res = Searcher::delete(&mut writer, &doc_id);
                                    if let Err(err) = res {
                                        log::error!("Unable to remove docs: {:?}", err);
                                    }
                                }
                            }

                            log::info!("removed {} docs from indexed_documents", num_removed);
                        }
                    }
                }
            }
        }
    }

    log::info!("✅ finished lens checks")
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
        assert!(!check_and_bootstrap(&Default::default(), &db, &settings, &test).await);
    }
}
