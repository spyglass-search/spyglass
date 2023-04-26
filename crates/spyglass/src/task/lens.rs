use dashmap::DashMap;
use entities::models::lens;
use shared::response::InstallableLens;
use std::fs;
use std::path::PathBuf;

use crate::{
    state::AppState,
    task::{CollectTask, ManagerCommand},
};
use reqwest::Client;
use shared::config::{Config, LensConfig, LensSource};
use shared::constants;

/// Loop through lenses in the AppState. Update our internal db & bootstrap anything
/// that hasn't been bootstrapped.
pub async fn load_lenses(lens_map: &DashMap<String, LensConfig>, state: AppState) {
    let mut new_lenses: Vec<LensConfig> = Vec::new();
    for entry in lens_map.iter() {
        let mut lens = entry.value().clone();
        // Have we added this lens to the database?
        match lens::add_or_enable(&state.db, &lens, lens::LensType::Simple).await {
            Ok((is_new, model)) => {
                log::debug!("model? {:?}", model);
                log::info!("loaded lens {}, new? {}", lens.name, is_new);
                match model.remote_url {
                    Some(url) => {
                        lens.lens_source = LensSource::Remote(url);
                    }
                    None => {
                        lens.lens_source = LensSource::Local;
                    }
                }
                if is_new {
                    state.lenses.insert(lens.name.to_owned(), lens.clone());
                    new_lenses.push(lens);
                } else if !state.lenses.contains_key(&lens.name) {
                    state.lenses.insert(lens.name.to_owned(), lens.clone());
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
        let _ = state
            .schedule_work(ManagerCommand::Collect(CollectTask::BootstrapLens {
                lens: lens.name.to_owned(),
            }))
            .await;
    }

    log::info!("✅ finished lens checks")
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
    let config = LensConfig::from_string(&file_contents)?;
    // File name should match lens name for consistency
    let file_name = format!("{}.ron", config.name);

    // Add to database
    let (is_new, model) = lens::install_or_update(
        &state.db,
        &config,
        lens::LensType::Simple,
        Some(installable_lens.html_url.clone()),
    )
    .await?;

    log::debug!("add {} to db: {:?}", config.name, model);
    log::info!("Installed new lens {}, new? {}", config.name, is_new);

    // Find and remove the old lens
    if let Ok(list) = std::fs::read_dir(lens_folder.clone()) {
        for file in list.flatten() {
            let path = file.path();
            if path.is_file() {
                if let Ok(lens) = ron::de::from_str::<LensConfig>(
                    &std::fs::read_to_string(path.clone()).unwrap_or_default(),
                ) {
                    if lens.name == installable_lens.name {
                        let _ = std::fs::remove_file(path);
                        break;
                    }
                }
            }
        }
    }

    // Write to disk if we've successfully add to the database
    fs::write(lens_folder.join(file_name), file_contents)?;
    Ok(())
}

/// Reads lens directly from disk and provides the map lenses
pub async fn read_lenses(config: &Config) -> anyhow::Result<DashMap<String, LensConfig>> {
    let lens_map = DashMap::new();
    let lense_dir = config.lenses_dir();

    // Keep track of failures and report to user?
    for entry in (fs::read_dir(lense_dir)?).flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().unwrap_or_default() == "ron" {
            match LensConfig::from_path(path) {
                Err(err) => log::warn!("Unable to load lens {:?}: {}", entry.path(), err),
                Ok(lens) => {
                    if lens.is_enabled {
                        lens_map.insert(lens.name.clone(), lens);
                    }
                }
            }
        }
    }

    Ok(lens_map)
}
