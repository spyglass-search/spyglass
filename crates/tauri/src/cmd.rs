use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{atomic::Ordering, Arc};

use tauri::api::dialog::FileDialogBuilder;
use tauri::Manager;
use tauri::State;

use crate::plugins::lens_updater::install_lens_to_path;
use crate::PauseState;
use crate::{open_folder, rpc, window};
use shared::config::{Config, Limit, UserSettings};
use shared::{event::ClientEvent, form::SettingOpts, request, response};
use spyglass_rpc::RpcClient;

#[tauri::command]
pub async fn authorize_connection(win: tauri::Window, name: String) -> Result<(), String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        if let Err(err) = rpc.client.authorize_connection(name).await {
            return Err(err.to_string());
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn choose_folder(win: tauri::Window) -> Result<(), String> {
    FileDialogBuilder::new().pick_folder(move |folder_path| {
        if let Some(folder_path) = folder_path {
            let _ = win.emit(ClientEvent::FolderChosen.as_ref(), folder_path);
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn escape(window: tauri::Window) -> Result<(), String> {
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
pub async fn open_plugins_folder(
    _: tauri::Window,
    config: State<'_, Config>,
) -> Result<(), String> {
    open_folder(config.plugins_dir());
    Ok(())
}

#[tauri::command]
pub async fn open_settings_folder(_: tauri::Window) -> Result<(), String> {
    open_folder(Config::prefs_dir());
    Ok(())
}

#[tauri::command]
pub async fn open_result(_: tauri::Window, url: &str) -> Result<(), String> {
    if let Ok(mut url) = url::Url::parse(url) {
        // treat open files as a local action.
        if url.scheme() == "file" {
            let _ = url.set_host(None);

            #[cfg(target_os = "windows")]
            {
                use shared::url_to_file_path;
                let path = url_to_file_path(url.path(), true);
                open::that(format!("file://{}", path)).unwrap();
                return Ok(());
            }
        }

        open::that(url.to_string()).unwrap();
    }
    Ok(())
}

#[tauri::command]
pub async fn resize_window(window: tauri::Window, height: f64) {
    window::resize_window(&window, height).await;
}

#[tauri::command]
pub async fn crawl_stats<'r>(win: tauri::Window) -> Result<response::CrawlStats, String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        match rpc.client.crawl_stats().await {
            Ok(resp) => Ok(resp),
            Err(err) => {
                log::error!("Error sending RPC: {}", err);
                Ok(response::CrawlStats {
                    by_domain: Vec::new(),
                })
            }
        }
    } else {
        Ok(response::CrawlStats {
            by_domain: Vec::new(),
        })
    }
}

#[tauri::command]
pub async fn search_docs<'r>(
    win: tauri::Window,
    lenses: Vec<String>,
    query: &str,
) -> Result<Vec<response::SearchResult>, String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let data = request::SearchParam {
            lenses,
            query: query.to_string(),
        };

        let rpc = rpc.lock().await;
        match rpc.client.search_docs(data).await {
            Ok(resp) => Ok(resp.results.to_vec()),
            Err(err) => {
                log::error!("search_docs err: {}", err);
                Ok(Vec::new())
            }
        }
    } else {
        Ok(Vec::new())
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
        match rpc.client.delete_doc(id.to_string()).await {
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
pub async fn delete_domain<'r>(window: tauri::Window, domain: &str) -> Result<(), String> {
    if let Some(rpc) = window.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        match rpc.client.delete_domain(domain.to_string()).await {
            Ok(_) => {
                let _ = window.emit(ClientEvent::RefreshSearchResults.as_ref(), true);
            }
            Err(err) => {
                log::error!("delete_domain err: {}", err);
            }
        }
    }

    Ok(())
}

/// Install a lens (assumes correct format) from a URL
#[tauri::command]
pub async fn install_lens<'r>(
    window: tauri::Window,
    config: State<'_, Config>,
    download_url: &str,
) -> Result<(), String> {
    if let Err(e) = install_lens_to_path(download_url, config.lenses_dir()).await {
        log::error!(
            "Unable to install lens {}, due to error: {}",
            download_url,
            e
        );
    } else {
        // Sleep for a second to let the app reload the lenses and then let the client know we're done.
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        let _ = window.emit(ClientEvent::RefreshLensManager.as_ref(), true);
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
pub async fn list_connections(
    win: tauri::Window,
) -> Result<Vec<response::ConnectionResult>, String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        match rpc.client.list_connections().await {
            Ok(connections) => Ok(connections),
            Err(err) => {
                log::error!("list_connections err: {}", err.to_string());
                Ok(Vec::new())
            }
        }
    } else {
        Ok(Vec::new())
    }
}

#[tauri::command]
pub async fn list_plugins(win: tauri::Window) -> Result<Vec<response::PluginResult>, String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        match rpc.client.list_plugins().await {
            Ok(plugins) => Ok(plugins),
            Err(err) => {
                log::error!("list_plugins err: {}", err.to_string());
                Ok(Vec::new())
            }
        }
    } else {
        Ok(Vec::new())
    }
}

#[tauri::command]
pub async fn toggle_plugin(window: tauri::Window, name: &str) -> Result<(), String> {
    if let Some(rpc) = window.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        let _ = rpc.client.toggle_plugin(name.to_string()).await;
        let _ = window.emit(ClientEvent::RefreshPluginManager.as_ref(), true);
    }

    Ok(())
}

#[tauri::command]
pub async fn save_user_settings(
    window: tauri::Window,
    config: State<'_, Config>,
    settings: HashMap<String, String>,
) -> Result<(), HashMap<String, String>> {
    let mut current_settings = config.user_settings.clone();

    let config_list: Vec<(String, SettingOpts)> = config.user_settings.clone().into();
    let setting_configs: HashMap<String, SettingOpts> = config_list.into_iter().collect();
    let mut errors: HashMap<String, String> = HashMap::new();

    let plugin_configs = config.load_plugin_config();

    let mut fields_updated: usize = 0;

    // Loop through each updated settings value sent from the front-end and
    // validate the values.
    for (key, value) in settings.iter() {
        // Update spyglass or plugin settings?
        if let Some((parent, field)) = key.split_once('.') {
            match parent {
                // Hacky way to update user settings directly.
                "_" => {
                    if let Some(opt) = setting_configs.get(key) {
                        match opt.form_type.validate(value) {
                            Ok(val) => {
                                fields_updated += 1;
                                match field {
                                    "data_directory" => {
                                        current_settings.data_directory = PathBuf::from(val);
                                    }
                                    "disable_autolaunch" => {
                                        current_settings.disable_autolaunch =
                                            serde_json::from_str(value).unwrap_or_default();
                                    }
                                    "disable_telemetry" => {
                                        current_settings.disable_telemetry =
                                            serde_json::from_str(value).unwrap_or_default();
                                    }
                                    "inflight_crawl_limit" => {
                                        let limit: u32 = serde_json::from_str(value).unwrap_or(10);
                                        current_settings.inflight_crawl_limit =
                                            Limit::Finite(limit);
                                    }
                                    "inflight_domain_limit" => {
                                        let limit: u32 = serde_json::from_str(value).unwrap_or(2);
                                        current_settings.inflight_domain_limit =
                                            Limit::Finite(limit);
                                    }
                                    "port" => {
                                        current_settings.port = serde_json::from_str(value)
                                            .unwrap_or_else(|_| UserSettings::default_port());
                                    }
                                    _ => {}
                                }
                            }
                            Err(err) => {
                                errors.insert(key.to_string(), err);
                            }
                        }
                    }
                }
                plugin_name => {
                    // Load plugin settings configurations
                    let plugin_config = plugin_configs
                        .get(plugin_name)
                        .expect("Unable to find plugin");

                    if let Some(to_update) = current_settings.plugin_settings.get_mut(plugin_name) {
                        if let Some(field_opts) = plugin_config.user_settings.get(field) {
                            // Validate & serialize value into something we can save.
                            match field_opts.form_type.validate(value) {
                                Ok(val) => {
                                    fields_updated += 1;
                                    to_update.insert(field.into(), val);
                                }
                                Err(err) => {
                                    errors.insert(key.to_string(), err);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Only save settings if everything is valid.
    if errors.is_empty() && fields_updated > 0 {
        let _ = config.save_user_settings(&current_settings);
        let app = window.app_handle();
        app.restart();
        Ok(())
    } else {
        Err(errors)
    }
}

#[tauri::command]
pub async fn load_user_settings(
    _: tauri::Window,
    config: State<'_, Config>,
) -> Result<Vec<(String, SettingOpts)>, String> {
    let current_settings = Config::load_user_settings().expect("Unable to read user settings");

    let plugin_configs = config.load_plugin_config();
    let mut list: Vec<(String, SettingOpts)> = current_settings.clone().into();

    let current_plug_settings = current_settings.plugin_settings;
    for (pname, pconfig) in plugin_configs {
        for (setting_name, setting_opts) in pconfig.user_settings {
            let mut opts = setting_opts.clone();

            let value = current_plug_settings
                .get(&pname)
                .and_then(|settings| settings.get(&setting_name))
                // Reverse backslash escaping
                .map(|value| value.to_string().replace("\\\\", "\\"));

            if let Some(value) = value {
                opts.value = value.to_string();
            }

            list.push((format!("{}.{}", pname, setting_name), opts));
        }
    }

    list.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(list)
}

#[tauri::command]
pub async fn update_and_restart(window: tauri::Window) -> Result<(), String> {
    let app_handle = window.app_handle();
    if let Ok(updater) = app_handle.updater().check().await {
        log::info!("downloading new update...");
        if let Err(e) = updater.download_and_install().await {
            window::alert(&window, "Unable to update", &e.to_string());
        } else {
            log::info!("completed update, restarting!");
            app_handle.restart();
        }
    }
    Ok(())
}
