use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use jsonrpc_core::Value;
use tauri::Manager;
use tauri::State;
use url::Url;

use crate::{constants, open_folder, rpc, window};
use shared::{
    config::Config,
    event::ClientEvent,
    request,
    response::{self, InstallableLens},
    FormType, SettingOpts,
};

#[tauri::command]
pub async fn escape(window: tauri::Window) -> Result<(), String> {
    window::hide_window(&window);
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
pub async fn crawl_stats<'r>(
    _: tauri::Window,
    rpc: State<'_, rpc::RpcMutex>,
) -> Result<response::CrawlStats, String> {
    let mut rpc = rpc.lock().await;
    match rpc
        .client
        .call_method::<Value, response::CrawlStats>("crawl_stats", "", Value::Null)
        .await
    {
        Ok(resp) => Ok(resp),
        Err(err) => {
            log::error!("Error sending RPC: {}", err);
            rpc.reconnect().await;
            Ok(response::CrawlStats {
                by_domain: Vec::new(),
            })
        }
    }
}

#[tauri::command]
pub async fn list_installed_lenses(
    _: tauri::Window,
    rpc: State<'_, rpc::RpcMutex>,
) -> Result<Vec<response::LensResult>, String> {
    let mut rpc = rpc.lock().await;
    Ok(rpc
        .call::<Value, Vec<response::LensResult>>("list_installed_lenses", Value::Null)
        .await)
}

#[tauri::command]
pub async fn list_installable_lenses(
    _: tauri::Window,
) -> Result<Vec<response::InstallableLens>, String> {
    let client = reqwest::Client::builder()
        .user_agent(constants::APP_USER_AGENT)
        .build()
        .expect("Unable to create reqwest client");

    if let Ok(res) = client.get(constants::LENS_DIRECTORY_INDEX_URL).send().await {
        if let Ok(file_contents) = res.text().await {
            return match ron::from_str::<Vec<InstallableLens>>(&file_contents) {
                Ok(json) => Ok(json),
                Err(e) => Err(format!("Unable to parse index: {}", e)),
            };
        }
    }

    Ok(Vec::new())
}

#[tauri::command]
pub async fn search_docs<'r>(
    _: tauri::Window,
    rpc: State<'r, rpc::RpcMutex>,
    lenses: Vec<String>,
    query: &str,
) -> Result<Vec<response::SearchResult>, String> {
    let data = request::SearchParam {
        lenses,
        query: query.to_string(),
    };

    let rpc = rpc.lock().await;
    match rpc
        .client
        .call_method::<(request::SearchParam,), response::SearchResults>("search_docs", "", (data,))
        .await
    {
        Ok(resp) => Ok(resp.results.to_vec()),
        Err(err) => {
            log::error!("rpc resp {}", err);
            Ok(Vec::new())
        }
    }
}

#[tauri::command]
pub async fn search_lenses<'r>(
    _: tauri::Window,
    rpc: State<'r, rpc::RpcMutex>,
    query: &str,
) -> Result<Vec<response::LensResult>, String> {
    let data = request::SearchLensesParam {
        query: query.to_string(),
    };

    let mut rpc = rpc.lock().await;
    let resp = rpc
        .call::<(request::SearchLensesParam,), response::SearchLensesResp>("search_lenses", (data,))
        .await;
    Ok(resp.results)
}

#[tauri::command]
pub async fn delete_doc<'r>(
    window: tauri::Window,
    rpc: State<'_, rpc::RpcMutex>,
    id: &str,
) -> Result<(), String> {
    let mut rpc = rpc.lock().await;
    match rpc
        .client
        .call_method::<(String,), ()>("delete_doc", "", (id.into(),))
        .await
    {
        Ok(_) => {
            let _ = window.emit(ClientEvent::RefreshSearchResults.as_ref(), true);
            Ok(())
        }
        Err(err) => {
            log::error!("Error sending RPC: {}", err);
            rpc.reconnect().await;
            Ok(())
        }
    }
}

#[tauri::command]
pub async fn delete_domain<'r>(
    window: tauri::Window,
    rpc: State<'_, rpc::RpcMutex>,
    domain: &str,
) -> Result<(), String> {
    let mut rpc = rpc.lock().await;
    match rpc
        .client
        .call_method::<(String,), ()>("delete_domain", "", (domain.into(),))
        .await
    {
        Ok(_) => {
            let _ = window.emit(ClientEvent::RefreshSearchResults.as_ref(), true);
            Ok(())
        }
        Err(err) => {
            log::error!("Error sending RPC: {}", err);
            rpc.reconnect().await;
            Ok(())
        }
    }
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
    _: tauri::Window,
    rpc: State<'_, rpc::RpcMutex>,
    is_offline: bool,
) -> Result<(), String> {
    log::info!(
        "network change detected ({}), toggling crawler",
        if is_offline { "offline" } else { "online" }
    );

    if is_offline {
        let rpc = rpc.lock().await;
        let _ = rpc
            .client
            .call_method::<(bool,), ()>("toggle_pause", "", (true,))
            .await;
    }

    Ok(())
}

#[tauri::command]
pub async fn recrawl_domain(
    _: tauri::Window,
    rpc: State<'_, rpc::RpcMutex>,
    domain: &str,
) -> Result<(), String> {
    log::info!("recrawling {}", domain);
    let mut rpc = rpc.lock().await;

    match rpc
        .client
        .call_method::<(String,), ()>("recrawl_domain", "", (domain.into(),))
        .await
    {
        Ok(_) => Ok(()),
        Err(err) => {
            log::error!("Error sending RPC: {}", err);
            rpc.reconnect().await;
            Ok(())
        }
    }
}

#[tauri::command]
pub async fn list_plugins(
    _: tauri::Window,
    rpc: State<'_, rpc::RpcMutex>,
) -> Result<Vec<response::PluginResult>, String> {
    let mut rpc = rpc.lock().await;
    Ok(rpc
        .call::<Value, Vec<response::PluginResult>>("list_plugins", Value::Null)
        .await)
}

#[tauri::command]
pub async fn toggle_plugin(
    window: tauri::Window,
    rpc: State<'_, rpc::RpcMutex>,
    name: &str,
) -> Result<(), String> {
    let mut rpc = rpc.lock().await;
    rpc.call::<(String,), ()>("toggle_plugin", (name.into(),))
        .await;
    let _ = window.emit(ClientEvent::RefreshPluginManager.as_ref(), true);

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

    // Update the user settings
    for (key, value) in settings.iter() {
        if let Some((parent, field)) = key.split_once('.') {
            match parent {
                // Hacky way to update user settings directly.
                "_" => {
                    if field == "data_directory" {
                        user_settings.data_directory = PathBuf::from(value);
                    }
                }
                plugin_name => {
                    let plugin_config = plugin_configs
                        .get(plugin_name)
                        .expect("Unable to find plugin");

                    if let Some(to_update) = user_settings.plugin_settings.get_mut(plugin_name) {
                        if let Some(field_opts) = plugin_config.user_settings.get(field) {
                            let value = match field_opts.form_type {
                                FormType::Text => Some(value.into()),
                                FormType::List => {
                                    // Escape backslashes
                                    let value = value.replace('\\', "\\\\");
                                    // Validate the value by attempting to deserialize
                                    match serde_json::from_str::<Vec<String>>(&value) {
                                        Ok(parsed) => {
                                            serde_json::to_string::<Vec<String>>(&parsed).ok()
                                        }
                                        Err(e) => {
                                            window::alert(
                                                &window,
                                                "Unable to save settings",
                                                &format!("Reason: {}", e),
                                            );
                                            received_error = true;
                                            log::error!("unable to save setting: {}", e);
                                            None
                                        }
                                    }
                                }
                            };

                            if let Some(value) = value {
                                to_update.insert(field.into(), value);
                            }
                        }
                    }
                }
            }
        }
    }

    // Only save settings if everything is valid.
    if !received_error {
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
