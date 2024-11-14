use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::{atomic::Ordering, Arc};

use shared::response::{DefaultIndices, SearchResults};
use tauri::Manager;
use tauri::{Emitter, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::UpdaterExt;

use crate::constants::TabLocation;
use crate::current_version;
use crate::window::navigate_to_tab;
use crate::PauseState;
use crate::{open_folder, rpc, window};
use shared::config::{Config, UserSettings};
use shared::metrics::{Event, Metrics};
use shared::{event::ClientEvent, request, response};
use spyglass_rpc::RpcClient;

use super::platform::os_open;

mod settings;
pub use settings::*;

#[tauri::command]
pub async fn authorize_connection(win: tauri::Window, id: String) -> Result<(), String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        if let Err(err) = rpc.client.authorize_connection(id).await {
            return Err(err.to_string());
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn choose_folder(win: tauri::Window) -> Result<Option<String>, String> {
    let app = win.app_handle();
    let folder_path = app.dialog().file().blocking_pick_folder();
    let folder_path = folder_path.map(|x| x.to_string());
    Ok(folder_path)
}

#[tauri::command]
pub async fn escape(window: tauri::WebviewWindow) -> Result<(), String> {
    window::hide_search_bar(&window);
    Ok(())
}

#[tauri::command]
pub async fn open_folder_path(_: tauri::Window, path: &str) -> Result<(), String> {
    open_folder(Path::new(path).to_path_buf());
    Ok(())
}

#[tauri::command]
pub async fn open_lens_folder(_: tauri::Window, config: State<'_, Config>) -> Result<(), String> {
    open_folder(config.lenses_dir());
    Ok(())
}

#[tauri::command]
pub async fn open_settings_folder(_: tauri::Window) -> Result<(), String> {
    open_folder(Config::prefs_dir());
    Ok(())
}

#[tauri::command]
pub async fn open_result(
    win: tauri::Window,
    url: &str,
    application: Option<String>,
) -> Result<(), String> {
    let mut schema = String::from("unknown");
    let mut is_default_action = false;

    let application = if application
        .as_ref()
        .is_some_and(|a| a.to_lowercase() != "default")
    {
        application
    } else {
        None
    };

    let action = if application.is_some() {
        "open_application"
    } else {
        is_default_action = true;
        "open_url"
    };

    let result = match url::Url::parse(url) {
        Ok(mut url) => {
            schema = String::from(url.scheme());
            if url.scheme() == "file" {
                let _ = url.set_host(None);
            }

            if let Err(err) = os_open(&url, application) {
                log::warn!("Unable to open {} due to: {}", url.to_string(), err);
                return Err(err.to_string());
            }
            Ok(())
        }
        Err(err) => Err(err.to_string()),
    };

    if let Some(metrics) = win.try_state::<Metrics>() {
        metrics
            .track(Event::ResultActionTriggered {
                action: String::from(action),
                is_default_action,
                schema,
            })
            .await;
    }

    result
}

#[tauri::command]
pub async fn copy_to_clipboard(win: tauri::Window, txt: &str) -> Result<(), String> {
    if let Err(error) = win.app_handle().clipboard().write_text(String::from(txt)) {
        log::warn!("Error copying content to clipboard {:?}", error);
        return Err(error.to_string());
    }

    if let Some(metrics) = win.try_state::<Metrics>() {
        metrics
            .track(Event::ResultActionTriggered {
                action: String::from("copy_to_clipboard"),
                is_default_action: false,
                schema: String::from("unknown"),
            })
            .await;
    }
    Ok(())
}

#[tauri::command]
pub async fn resize_window(window: tauri::WebviewWindow, height: f64) {
    window::resize_window(&window, height).await;
}

#[tauri::command]
pub async fn search_docs<'r>(
    win: tauri::Window,
    lenses: Vec<String>,
    query: &str,
) -> Result<SearchResults, String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let data = request::SearchParam {
            lenses,
            query: query.to_string(),
        };

        let rpc = rpc.lock().await;
        match rpc.client.search_docs(data).await {
            Ok(resp) => Ok(resp),
            Err(err) => {
                log::error!("search_docs err: {}", err);
                Err(err.to_string())
            }
        }
    } else {
        Err("Unable to reach backend".to_string())
    }
}

#[tauri::command]
pub async fn search_lenses<'r>(
    win: tauri::Window,
    query: &str,
) -> Result<Vec<response::LensResult>, String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let data = request::SearchLensesParam {
            query: query.to_string(),
        };

        let rpc = rpc.lock().await;
        match rpc.client.search_lenses(data).await {
            Ok(resp) => Ok(resp.results),
            Err(err) => {
                log::error!("search_lenses err: {}", err.to_string());
                Ok(Vec::new())
            }
        }
    } else {
        Ok(Vec::new())
    }
}

#[tauri::command]
pub async fn delete_doc<'r>(window: tauri::Window, id: &str) -> Result<(), String> {
    if let Some(rpc) = window.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        match rpc.client.delete_document(id.to_string()).await {
            Ok(_) => {
                let _ = window.emit(ClientEvent::RefreshSearchResults.as_ref(), true);
            }
            Err(err) => {
                log::error!("delete_doc err: {}", err);
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn network_change(
    win: tauri::Window,
    paused: State<'_, Arc<PauseState>>,
    is_offline: bool,
) -> Result<(), String> {
    log::info!(
        "network change detected ({}), toggling crawler",
        if is_offline { "offline" } else { "online" }
    );

    if is_offline {
        if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
            let rpc = rpc.lock().await;
            paused.store(true, Ordering::Relaxed);
            let _ = rpc.client.toggle_pause(true).await;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn recrawl_domain(win: tauri::Window, domain: &str) -> Result<(), String> {
    log::info!("recrawling {}", domain);
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        if let Err(err) = rpc.client.recrawl_domain(domain.to_string()).await {
            log::error!("recrawl_domain err: {}", err);
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_library_stats(
    win: tauri::Window,
) -> Result<HashMap<String, response::LibraryStats>, String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        match rpc.client.get_library_stats().await {
            Ok(res) => Ok(res),
            Err(err) => {
                log::error!("get_library_stats err: {}", err.to_string());
                Err(err.to_string())
            }
        }
    } else {
        Err("Unable to communicate w/ backend".to_string())
    }
}

#[tauri::command]
pub async fn list_connections(
    win: tauri::Window,
) -> Result<response::ListConnectionResult, String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        match rpc.client.list_connections().await {
            Ok(connections) => Ok(connections),
            Err(err) => {
                log::error!("list_connections err: {}", err.to_string());
                Err(err.to_string())
            }
        }
    } else {
        Err("Unable to communicate w/ backend".to_string())
    }
}

#[tauri::command]
pub async fn update_and_restart(window: tauri::WebviewWindow) -> Result<(), String> {
    let app_handle = window.app_handle();
    if let Ok(Some(update)) = app_handle
        .updater()
        .map_err(|err| err.to_string())?
        .check()
        .await
    {
        log::info!("downloading new update...");
        update
            .download_and_install(
                |_chunk_length, _content_length| {},
                || {
                    log::info!("completed update, restarting!");
                    app_handle.restart();
                },
            )
            .await
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn revoke_connection(win: tauri::Window, id: &str, account: &str) -> Result<(), String> {
    log::debug!("revoking connection: {}@{}", account, id);
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        if let Err(err) = rpc
            .client
            .revoke_connection(id.to_string(), account.to_string())
            .await
        {
            return Err(err.to_string());
        } else {
            let _ = win.emit(ClientEvent::RefreshConnections.as_ref(), true);
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn resync_connection(win: tauri::Window, id: &str, account: &str) -> Result<(), String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        if let Err(err) = rpc
            .client
            .resync_connection(id.to_string(), account.to_string())
            .await
        {
            return Err(err.to_string());
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_shortcut(win: tauri::Window) -> Result<String, String> {
    if let Some(config) = win.app_handle().try_state::<Config>() {
        Ok(config.user_settings.shortcut.clone())
    } else {
        Ok("CmdOrCtrl+Shift+/".to_string())
    }
}

#[tauri::command]
pub async fn wizard_finished(
    win: tauri::Window,
    config: State<'_, Config>,
    toggle_audio_transcription: bool,
    toggle_file_indexer: bool,
) -> Result<(), String> {
    let mut current_settings = config.user_settings.clone();
    current_settings.run_wizard = true;

    current_settings
        .filesystem_settings
        .enable_filesystem_scanning = toggle_file_indexer;

    current_settings.audio_settings.enable_audio_transcription = toggle_audio_transcription;

    if let Some(metrics) = win.app_handle().try_state::<Metrics>() {
        let current_version = current_version(win.app_handle().package_info());
        metrics
            .track(Event::WizardFinished { current_version })
            .await;
    }

    if let Err(error) = update_user_settings(win.clone(), &current_settings).await {
        log::error!("Error saving initial settings {:?}", error);
    }

    // close wizard window
    if let Some(window) = win.get_webview_window(crate::constants::WIZARD_WIN_NAME) {
        let _ = window.close();
        navigate_to_tab(
            window.app_handle(),
            &crate::constants::TabLocation::Discover,
        );
    }

    Ok(())
}

#[tauri::command]
pub async fn default_indices(win: tauri::Window) -> Result<DefaultIndices, String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        match rpc.client.default_indices().await {
            Ok(res) => return Ok(res),
            Err(err) => {
                log::info!("default_indices: {:?}", err);
            }
        }
    }

    Ok(DefaultIndices {
        file_paths: Vec::new(),
        extensions: Vec::new(),
    })
}

#[tauri::command]
pub async fn update_user_settings(
    win: tauri::Window,
    settings: &UserSettings,
) -> Result<UserSettings, String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        return match rpc.client.update_user_settings(settings.clone()).await {
            Ok(settings) => {
                return Ok(settings);
            }
            Err(error) => Err(error.to_string()),
        };
    }

    Err(String::from("Unable to access user settings"))
}

#[tauri::command]
pub async fn user_settings(win: tauri::Window) -> Result<UserSettings, String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        return match rpc.client.user_settings().await {
            Ok(settings) => {
                return Ok(settings);
            }
            Err(error) => Err(error.to_string()),
        };
    }

    Err(String::from("Unable to access user settings"))
}

#[tauri::command]
pub async fn navigate(win: tauri::Window, page: String) -> Result<(), String> {
    if let Ok(tab_loc) = TabLocation::from_str(&page) {
        super::window::navigate_to_tab(win.app_handle(), &tab_loc);
    }

    Ok(())
}
