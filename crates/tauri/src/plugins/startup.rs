// use std::sync::Arc;
use num_format::{Locale, ToFormattedString};
use serde_json::Value;
use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, RunEvent, Wry,
};
// use tokio::sync::Mutex;
use tokio::time::{self, Duration};

use shared::response;
use shared::response::AppStatus;

use crate::constants;
use crate::{
    menu::MenuID,
    rpc::{self, RpcMutex},
};

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("tauri-plugin-startup")
        .on_event(|app_handle, event| match event {
            RunEvent::Ready => {
                log::info!("ADLKDJAF");
                run_and_check_backend(app_handle);
            }
            _ => {}
        })
        .build()
}

fn run_and_check_backend(app_handle: &AppHandle) {
    // Wait for the server to boot up
    // let rpc = tauri::async_runtime::block_on(rpc::RpcClient::new());
    // app.manage(Arc::new(Mutex::new(rpc)));

    // Keep system tray stats updated
    // let app_handle = app_handle.clone();
    // tauri::async_runtime::spawn(async move {
    //     let mut interval = time::interval(Duration::from_secs(10));
    //     loop {
    //         update_tray_menu(&app_handle).await;
    //         interval.tick().await;
    //     }
    // });
}

async fn update_tray_menu(app: &AppHandle) {
    let rpc = app.state::<RpcMutex>().inner();
    let app_status: Option<AppStatus> = app_status(rpc).await;
    let handle = app.tray_handle();

    if let Some(app_status) = app_status {
        let _ = handle
            .get_item(&MenuID::NUM_DOCS.to_string())
            .set_title(format!(
                "{} documents indexed",
                app_status.num_docs.to_formatted_string(&Locale::en)
            ));
    }
}

async fn app_status(rpc: &rpc::RpcMutex) -> Option<response::AppStatus> {
    let mut rpc = rpc.lock().await;
    match rpc
        .client
        .call_method::<Value, response::AppStatus>("app_status", "", Value::Null)
        .await
    {
        Ok(resp) => Some(resp),
        Err(err) => {
            log::error!("Error sending RPC: {}", err);
            rpc.reconnect().await;
            None
        }
    }
}
