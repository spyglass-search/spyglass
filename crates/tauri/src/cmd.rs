use jsonrpc_core::Value;
use tauri::State;

use crate::{rpc, window};
use shared::{request, response};

#[tauri::command]
pub async fn escape(window: tauri::Window) -> Result<(), String> {
    window::hide_window(&window);
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
            log::error!("{}", err);
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

    let rpc = rpc.lock().await;
    match rpc
        .client
        .call_method::<(request::SearchLensesParam,), response::SearchLensesResp>(
            "search_lenses",
            "",
            (data,),
        )
        .await
    {
        Ok(resp) => Ok(resp.results.to_vec()),
        Err(err) => {
            log::error!("{}", err);
            Ok(Vec::new())
        }
    }
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
pub async fn delete_doc<'r>(
    _: tauri::Window,
    rpc: State<'_, rpc::RpcMutex>,
    id: &str,
) -> Result<(), String> {
    let mut rpc = rpc.lock().await;
    match rpc
        .client
        .call_method::<(String,), ()>("delete_docs", "", (id.into(),)).await {

        Ok(_) => Ok(()),
        Err(err) => {
            log::error!("Error sending RPC: {}", err);
            rpc.reconnect().await;
            Ok(())
        }
    }
}
