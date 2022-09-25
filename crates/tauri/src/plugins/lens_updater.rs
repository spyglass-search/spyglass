use tauri::{
    async_runtime::JoinHandle,
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, RunEvent, Wry,
};
use tokio::signal;
use tokio::time::{self, Duration};

use crate::constants;
use shared::response::InstallableLens;

pub struct LensWatcherHandle(JoinHandle<()>);

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("lens-updater")
        .invoke_handler(tauri::generate_handler![
            list_installable_lenses,
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
                    loop {
                        tokio::select! {
                            _ = signal::ctrl_c() => break,
                            _ = interval.tick() => check_for_lens_updates(&app_handle).await,
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

async fn check_for_lens_updates(_app_handle: &AppHandle) {
    // Get the latest lens index
    log::info!("check_for_lens_updates called");
}

async fn get_lens_index() -> anyhow::Result<Vec<InstallableLens>> {
    let resp = reqwest::get(constants::LENS_DIRECTORY_INDEX_URL).await?;
    let file_contents = resp.text().await?;

    match ron::from_str::<Vec<InstallableLens>>(&file_contents) {
        Ok(json) => Ok(json),
        Err(e) => Err(anyhow::anyhow!(format!("Unable to parse index: {}", e))),
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
pub async fn run_lens_updater(win: tauri::Window) -> Result<(), String> {
    check_for_lens_updates(&win.app_handle()).await;
    Ok(())
}
