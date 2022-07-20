use std::fs;

use jsonrpc_core::Value;
use tauri::State;
use url::Url;

use crate::{constants, open_folder, rpc, window};
use shared::{
    config::Config,
    request,
    response::{self, InstallableLens},
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
pub async fn open_result(_: tauri::Window, url: &str) -> Result<(), String> {
    open::that(url).unwrap();
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
            let _ = window.emit("refresh_results", true);
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
                let _ = window.emit("refresh_lens_manager", true);
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

    let mut rpc = rpc.lock().await;
    match rpc
        .client
        .call_method::<Value, response::AppStatus>("app_status", "", Value::Null)
        .await
    {
        Ok(status) => {
            // Pause the crawler if we're offline and we're currently crawling.
            let should_toggle =
                (!status.is_paused && is_offline) || (status.is_paused && !is_offline);

            if should_toggle {
                let _ = rpc
                    .client
                    .call_method::<Value, response::AppStatus>("toggle_pause", "", Value::Null)
                    .await;
            }
        }
        Err(err) => {
            log::error!("Error sending RPC: {}", err);
            rpc.reconnect().await;
        }
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
    let _ = window.emit("refresh_plugin_manager", true);

    Ok(())
}
