use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{atomic::Ordering, Arc};

use tauri::Manager;
use tauri::State;
use url::Url;

use crate::window::alert;
use crate::PauseState;
use crate::{constants, open_folder, rpc, window};
use shared::{
    config::Config,
    event::ClientEvent,
    form::{FormType, SettingOpts},
    request, response,
};
use spyglass_rpc::RpcClient;

#[tauri::command]
pub async fn escape(window: tauri::Window) -> Result<(), String> {
    window::hide_search_bar(&window);
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
pub async fn list_installed_lenses(
    win: tauri::Window,
) -> Result<Vec<response::LensResult>, String> {
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
    log::trace!("installing lens from <{}>", download_url);

    let client = reqwest::Client::builder()
        .user_agent(constants::APP_USER_AGENT)
        .build()
        .expect("Unable to create reqwest client");

    if let Ok(resp) = client.get(download_url).send().await {
        if let Ok(file_contents) = resp.text().await {
            // Grab the file name from the end of the URL
            let url = Url::parse(download_url).unwrap();
            let mut segments = url.path_segments().map(|c| c.collect::<Vec<_>>()).unwrap();
            let file_name = segments.pop().unwrap();
            // Create path from file name + lens directory
            let lens_path = config.lenses_dir().join(file_name);
            log::info!("installing lens to {:?}", lens_path);

            if let Err(e) = fs::write(lens_path.clone(), file_contents) {
                log::error!(
                    "Unable to install lens {} to {:?} due to error: {}",
                    download_url,
                    lens_path,
                    e
                );
            } else {
                // Sleep for a second to let the app reload the lenses and then let the client know we're done.
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                let _ = window.emit(ClientEvent::RefreshLensManager.as_ref(), true);
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
) -> Result<(), String> {
    let mut user_settings = config.user_settings.clone();
    let plugin_configs = config.load_plugin_config();

    let mut received_error = false;
    let mut fields_updated: usize = 0;

    // Loop through each updated settings value sent from the front-end and
    // validate the values.
    for (key, value) in settings.iter() {
        // Update spyglass or plugin settings?
        if let Some((parent, field)) = key.split_once('.') {
            match parent {
                // Hacky way to update user settings directly.
                "_" => {
                    if field == "data_directory" {
                        match FormType::Path.validate(value) {
                            Ok(val) => {
                                fields_updated += 1;
                                user_settings.data_directory = PathBuf::from(val);
                            }
                            Err(error) => {
                                // Show an alert
                                received_error = true;
                                alert(
                                    &window,
                                    "Error",
                                    &format!("Unable to save data directory due to: {}", error),
                                );
                            }
                        }
                    }
                }
                plugin_name => {
                    // Load plugin settings configurations
                    let plugin_config = plugin_configs
                        .get(plugin_name)
                        .expect("Unable to find plugin");

                    if let Some(to_update) = user_settings.plugin_settings.get_mut(plugin_name) {
                        if let Some(field_opts) = plugin_config.user_settings.get(field) {
                            // Validate & serialize value into something we can save.
                            match field_opts.form_type.validate(value) {
                                Ok(val) => {
                                    fields_updated += 1;
                                    to_update.insert(field.into(), val);
                                }
                                Err(error) => {
                                    // Show an alert
                                    received_error = true;
                                    alert(
                                        &window,
                                        "Error",
                                        &format!(
                                            "Unable to save {} due to: {}",
                                            field_opts.label, error
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Only save settings if everything is valid.
    if !received_error && fields_updated > 0 {
        let _ = config.save_user_settings(&user_settings);
        let app = window.app_handle();
        app.restart();
    }

    Ok(())
}

#[tauri::command]
pub async fn load_user_settings(
    _: tauri::Window,
    config: State<'_, Config>,
) -> Result<Vec<(String, SettingOpts)>, String> {
    let current_settings = Config::load_user_settings().expect("Unable to read user settings");

    let serialized: HashMap<String, String> = current_settings.clone().into();
    let plugin_configs = config.load_plugin_config();

    let mut list = vec![("_.data_directory".into(), SettingOpts {
        label: "Data Directory".into(),
        value: serialized.get("_.data_directory").unwrap_or(&"".to_string()).to_string(),
        form_type: FormType::Text,
        help_text: Some("The data directory is where your index, lenses, plugins, and logs are stored. This will require a restart.".into())
    })];

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
