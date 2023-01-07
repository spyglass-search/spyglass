use std::fs;
use entities::models::lens;
use entities::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use shared::config::{Config, LensConfig};
use spyglass_plugin::SearchFilter;

use crate::state::AppState;
use crate::task::{CollectTask, ManagerCommand};

/// Read lenses into the AppState
pub async fn read_lenses(state: &AppState, config: &Config) -> anyhow::Result<()> {
    state.lenses.clear();

    let lense_dir = config.lenses_dir();

    // Keep track of failures and report to user?
    for entry in (fs::read_dir(lense_dir)?).flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().unwrap_or_default() == "ron" {
            match LensConfig::from_path(path) {
                Err(err) => log::error!("Unable to load lens {:?}: {}", entry.path(), err),
                Ok(lens) => {
                    if lens.is_enabled {
                        state.lenses.insert(lens.name.clone(), lens);
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
    let mut new_lenses: Vec<LensConfig> = Vec::new();
    for entry in state.lenses.iter() {
        let lens = entry.value();
        // Have we added this lens to the database?
        match lens::add_or_enable(&state.db, lens, lens::LensType::Simple).await {
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
        log::debug!("Scheduling lens bootstrap {:?}", lens);
        let _ = state
            .schedule_work(ManagerCommand::Collect(CollectTask::BootstrapLens {
                lens: lens.name.clone(),
            }))
            .await;
    }

    log::info!("✅ finished lens checks")
}

/// Utility function to map a trigger to the matching lens(es) & convert that into
/// search filters ready to be applied to a search.
pub async fn lens_to_filters(state: AppState, trigger: &str) -> Vec<SearchFilter> {
    // Find the lenses that were triggered
    // NOTE: Users can combine lenses together but giving them the same trigger label
    let results = lens::Entity::find()
        .filter(lens::Column::Trigger.eq(trigger))
        .all(&state.db)
        .await
        .ok();

    // Based on the lens type, either use filters defined by the configuration
    // or ask the plugin for the search filter.
    let mut filters = Vec::new();
    for lens in results.unwrap_or_default() {
        match lens.lens_type {
            // Load lens configuration from files
            lens::LensType::Simple => {
                if let Some(lens_config) = state.lenses.get(&lens.name) {
                    let lens_filters = lens_config.into_regexes();
                    filters.extend(
                        lens_filters
                            .allowed
                            .into_iter()
                            .map(SearchFilter::URLRegexAllow)
                            .collect::<Vec<SearchFilter>>(),
                    );

                    filters.extend(
                        lens_filters
                            .skipped
                            .into_iter()
                            .map(SearchFilter::URLRegexSkip)
                            .collect::<Vec<SearchFilter>>(),
                    );
                }
            }
            // Ask plugin for any filter information
            lens::LensType::Plugin => {
                let manager = state.plugin_manager.lock().await;
                if let Some(plugin) = manager.find_by_name(lens.name) {
                    filters.extend(plugin.search_filters().await);
                }
            }
        }
    }

    if filters.is_empty() {
        // lens remove? Plugin disabled?
        log::warn!("No filters found for trigger: {}", trigger);
    }

    filters
}

#[cfg(test)]
mod test {
    use crate::search::IndexPath;
    use entities::models::lens;
    use entities::sea_orm::EntityTrait;
    use entities::test::setup_test_db;
    use shared::config::{LensConfig, UserSettings};
    use spyglass_plugin::SearchFilter;

    use super::{lens_to_filters, AppState};

    #[tokio::test]
    async fn test_lens_to_filter() {
        let db = setup_test_db().await;
        let test_lens = LensConfig {
            name: "test_lens".to_owned(),
            trigger: "test".to_owned(),
            urls: vec!["https://oldschool.runescape.wiki/wiki/".to_string()],
            ..Default::default()
        };

        if let Err(e) = lens::add_or_enable(&db, &test_lens, lens::LensType::Simple).await {
            eprintln!("{}", e);
        }

        // Make sure the lens was added
        let db_rows = lens::Entity::find().all(&db).await;
        assert_eq!(db_rows.unwrap().len(), 1);

        let state = AppState::builder()
            .with_db(db)
            .with_lenses(&vec![test_lens])
            .with_user_settings(&UserSettings::default())
            .with_index(&IndexPath::Memory)
            .build();

        let filters = lens_to_filters(state, "test").await;
        assert_eq!(filters.len(), 1);
        assert_eq!(
            *filters.get(0).unwrap(),
            SearchFilter::URLRegexAllow("^https://oldschool.runescape.wiki/wiki/.*".to_owned())
        );
    }
}
