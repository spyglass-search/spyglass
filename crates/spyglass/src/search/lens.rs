use anyhow::anyhow;
use entities::models::lens;
use entities::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use shared::response::InstallableLens;
use std::fs;
use std::path::PathBuf;

use shared::config::{Config, LensConfig, LensSource};
use spyglass_plugin::SearchFilter;

use crate::state::AppState;
use crate::task::{CollectTask, ManagerCommand};
use reqwest::Client;
use shared::constants;

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
        let mut lens = entry.value().clone();
        // Have we added this lens to the database?
        match lens::add_or_enable(&state.db, &lens, lens::LensType::Simple).await {
            Ok((is_new, model)) => {
                log::debug!("model? {:?}", model);
                log::info!("loaded lens {}, new? {}",lens.name, is_new);
                match model.remote_url {
                    Some(url) => {
                        lens.lens_source = LensSource::Remote(url);
                    }
                    None => {
                        lens.lens_source = LensSource::Local;
                    }
                }
                if is_new {
                    new_lenses.push(lens);
                }
            }
            Err(e) => log::error!("error loading lens {}", e),
        }
    }

    // Bootstrap lenses.
    // Check & bootstrap will go through domains/prefixes and bootstrap a crawl queue
    // if we have not already done so.
    for lens in new_lenses {
        log::debug!("Scheduling lens bootstrap {:?}", lens);

        let lens_name = lens.name.to_owned();
        //Since we reset the lens source update the lenses
        state.lenses.insert(lens_name.to_owned(), lens);
        let _ = state
            .schedule_work(ManagerCommand::Collect(CollectTask::BootstrapLens {
                lens: lens_name,
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

/// Installs a new lens or updates the current lens. The requested lens will be
/// downloaded from the lens store and added to the database. The actually lens
/// loading will happen through the normal file system watch mechanism.
pub async fn install_lens(
    app_state: &AppState,
    config: &Config,
    lens_name: String,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .user_agent(constants::APP_USER_AGENT)
        .build()
        .expect("Unable to create reqwest client");

    let resp = client
        .get(constants::LENS_DIRECTORY_INDEX_URL)
        .send()
        .await?;

    let file_contents = resp.text().await?;
    let available_lens = ron::from_str::<Vec<InstallableLens>>(&file_contents)?;

    let lens_data = available_lens
        .iter()
        .find(|installable_lens| installable_lens.name.eq(&lens_name));

    if let Some(installable_lens) = lens_data {
        // Check if there's an existing lens, and remove the old file.
        let current_lens_ref = app_state
            .lenses
            .iter()
            .find(|lens| lens.value().name.eq(&lens_name));

        if let Some(current_lens) = current_lens_ref {
            let path = &current_lens.value().file_path;
            if path.exists() {
                std::fs::remove_file(path)?;
            }
        }

        if let Err(e) =
            install_lens_to_path(app_state, &client, installable_lens, config.lenses_dir()).await
        {
            log::error!("Unable to install lens: {}", e);
            return Err(e);
        }
    }
    Ok(())
}

/// Helper method used to install the lens by downloading it to the specified
/// path. Before the lens is downloaded the database is updated with the new
/// lens entry. This database update allows the lens to be properly processed
/// later by the standard lens loading process.
async fn install_lens_to_path(
    state: &AppState,
    client: &Client,
    installable_lens: &InstallableLens,
    lens_folder: PathBuf,
) -> anyhow::Result<()> {
    log::info!("installing lens from <{}>", installable_lens.download_url);

    let resp = client
        .get(installable_lens.download_url.as_str())
        .send()
        .await?;
    let file_contents = resp.text().await?;
    let config_rslt = LensConfig::from_string(&file_contents);
    match config_rslt {
        Ok(config) => {
            // File name should match lens name for consistency
            let file_name = format!("{}.ron", config.name);

            let update = lens::install_or_update(
                &state.db,
                &config,
                lens::LensType::Simple,
                Some(installable_lens.html_url.clone()),
            )
            .await;

            match update {
                Ok((new, model)) => {
                    log::info!(
                        "Installed new lens {}, new? {}, model? {:?}",
                        config.name,
                        new,
                        model
                    );
                    fs::write(lens_folder.join(file_name), file_contents)?;
                    Ok(())
                }
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(anyhow!("Invalid len file configuration {:?}", error)),
    }
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
