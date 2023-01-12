use std::collections::HashMap;

use tauri::{
    async_runtime::JoinHandle,
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, RunEvent, Wry,
};
use tokio::sync::broadcast;
use tokio::time::{self, Duration};

use crate::{constants, rpc, AppShutdown};
use shared::event::ClientEvent;
use shared::response::{InstallableLens, LensResult};
use spyglass_rpc::RpcClient;

pub struct LensWatcherHandle(JoinHandle<()>);

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("lens-updater")
        .invoke_handler(tauri::generate_handler![
            list_installable_lenses,
            list_installed_lenses,
            run_lens_updater
        ])
        .on_event(|app_handle, event| match event {
            RunEvent::Ready => {
                let app_handle = app_handle.clone();
                let app_clone = app_handle.clone();
                let handle = tauri::async_runtime::spawn(async move {
                    let mut interval = time::interval(Duration::from_secs(
                        constants::LENS_UPDATE_CHECK_INTERVAL_S,
                    ));

                    let app_handle = app_handle.clone();
                    let shutdown_tx = app_handle.state::<broadcast::Sender<AppShutdown>>();
                    let mut shutdown = shutdown_tx.subscribe();

                    loop {
                        tokio::select! {
                            _ = shutdown.recv() => {
                                log::info!("🛑 Shutting down lens updater");
                                return;
                            },
                            _ = interval.tick() => {
                                let _ = check_for_lens_updates(&app_handle).await;
                            },
                        }
                    }
                });

                app_clone.manage(LensWatcherHandle(handle));
            }
            RunEvent::Exit => {
                let app_handle = app_handle.clone();
                if let Some(handle) = app_handle.try_state::<LensWatcherHandle>() {
                    handle.0.abort();
                }
            }
            _ => {}
        })
        .build()
}

async fn check_for_lens_updates(app_handle: &AppHandle) -> anyhow::Result<()> {
    // Get the latest lens index
    let lens_index = get_lens_index().await?;
    // Create a map from the index
    let mut lens_index_map: HashMap<String, InstallableLens> = HashMap::new();
    for lens in lens_index {
        lens_index_map.insert(lens.name.clone(), lens);
    }

    // Get installed lenses
    let installed = get_installed_lenses(app_handle).await?;

    // Loop through each one and check if it needs an update
    let mut lenses_updated = 0;
    for lens in installed {
        if lens_index_map.contains_key(&lens.title) {
            // Compare hash from index to local hash
            let latest = lens_index_map.get(&lens.title).expect("already checked");
            if latest.sha != lens.hash {
                log::info!(
                    "Found newer version of: {}, updating from: {}",
                    lens.title,
                    latest.download_url
                );

                if let Err(e) = install_lens(app_handle, &latest.name).await {
                    log::error!("Unable to install lens: {}", e);
                } else {
                    lenses_updated += 1;
                }
            }
        }
    }

    let _ = app_handle.emit_all(ClientEvent::UpdateLensFinished.as_ref(), true);
    log::info!("updated {} lenses", lenses_updated);
    Ok(())
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(shared::constants::APP_USER_AGENT)
        .build()
        .expect("Unable to create reqwest client")
}

async fn get_lens_index() -> anyhow::Result<Vec<InstallableLens>> {
    let client = http_client();
    let resp = client
        .get(shared::constants::LENS_DIRECTORY_INDEX_URL)
        .send()
        .await?;
    let file_contents = resp.text().await?;

    match ron::from_str::<Vec<InstallableLens>>(&file_contents) {
        Ok(json) => Ok(json),
        Err(e) => Err(anyhow::anyhow!(format!("Unable to parse index: {e}"))),
    }
}

async fn get_installed_lenses(app_handle: &AppHandle) -> anyhow::Result<Vec<LensResult>> {
    let mutex = app_handle
        .try_state::<rpc::RpcMutex>()
        .ok_or_else(|| anyhow::anyhow!("Unable to get RpcMutex"))?;

    let rpc = mutex.lock().await;
    match rpc.client.list_installed_lenses().await {
        Ok(lenses) => Ok(lenses),
        Err(err) => {
            log::error!("Unable to list installed lenses: {}", err.to_string());
            Ok(Vec::new())
        }
    }
}

/// Helper to install the lens with the specified name. A request to the server
/// will be made to install the lens.
pub async fn install_lens(app_handle: &AppHandle, lens_name: &String) -> anyhow::Result<()> {
    log::debug!("Lens install requested {}", lens_name);
    let mutex = app_handle
        .try_state::<rpc::RpcMutex>()
        .ok_or_else(|| anyhow::anyhow!("Unable to get RpcMutex"))?;

    let rpc = mutex.lock().await;
    match rpc.client.install_lens(lens_name.clone()).await {
        Ok(_) => Ok(()),
        Err(err) => {
            log::error!("Unable to install lens: {} {}", lens_name, err.to_string());
            Ok(())
        }
    }
}

#[tauri::command]
pub async fn list_installable_lenses(_: tauri::Window) -> Result<Vec<InstallableLens>, String> {
    match get_lens_index().await {
        Ok(index) => Ok(index),
        Err(err) => {
            log::error!("Unable to get lens index: {}", err);
            Ok(Vec::new())
        }
    }
}

#[tauri::command]
pub async fn list_installed_lenses(win: tauri::Window) -> Result<Vec<LensResult>, String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        match rpc.client.list_installed_lenses().await {
            Ok(lenses) => Ok(lenses),
            Err(err) => {
                log::error!("Unable to list installed lenses: {}", err.to_string());
                Ok(Vec::new())
            }
        }
    } else {
        Ok(Vec::new())
    }
}

#[tauri::command]
pub async fn run_lens_updater(win: tauri::Window) -> Result<(), String> {
    match check_for_lens_updates(&win.app_handle()).await {
        Ok(_) => Ok(()),
        Err(err) => {
            log::error!("Unable to run lens updater: {}", err);
            Ok(())
        }
    }
}
