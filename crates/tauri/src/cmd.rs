use shared::{request, response};
use tauri::{LogicalSize, Size, State};

use crate::_hide_window;
use crate::constants;
use crate::rpc;

#[tauri::command]
pub async fn escape(window: tauri::Window) -> Result<(), String> {
    _hide_window(&window);
    Ok(())
}

#[tauri::command]
pub async fn open_result(_: tauri::Window, url: &str) -> Result<(), String> {
    open::that(url).unwrap();
    Ok(())
}

#[tauri::command]
pub fn resize_window(window: tauri::Window, height: f64) {
    window
        .set_size(Size::Logical(LogicalSize {
            width: constants::INPUT_WIDTH,
            height,
        }))
        .unwrap();
}

#[tauri::command]
pub async fn search_docs<'r>(
    _: tauri::Window,
    rpc: State<'r, rpc::RpcClient>,
    lenses: Vec<String>,
    query: &str,
) -> Result<Vec<response::SearchResult>, String> {
    let data = request::SearchParam {
        lenses,
        query: query.to_string(),
    };

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
    rpc: State<'r, rpc::RpcClient>,
    query: &str,
) -> Result<Vec<response::LensResult>, String> {
    let data = request::SearchLensesParam {
        query: query.to_string(),
    };

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
