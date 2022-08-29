use num_format::{Locale, ToFormattedString};
use serde_json::Value;
use std::sync::Arc;
use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, RunEvent, Wry,
};
use tokio::sync::Mutex;
use tokio::time::{self, Duration};

use migration::Migrator;
use shared::response::AppStatus;
use shared::{event::ClientEvent, response};

use crate::{
    constants,
    menu::MenuID,
    rpc::{self, RpcMutex},
    window::alert,
};

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("tauri-plugin-startup")
        .on_event(|app_handle, event| {
            if let RunEvent::Ready = event {
                // Don't block the main thread
                tauri::async_runtime::spawn(run_and_check_backend(app_handle.clone()));

                // Keep system tray stats updated
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let mut interval = time::interval(Duration::from_secs(60));
                    loop {
                        update_tray_menu(&app_handle).await;
                        interval.tick().await;
                    }
                });
            }
        })
        .build()
}

async fn run_and_check_backend(app_handle: AppHandle) {
    log::info!("Running startup tasks");

    let window = app_handle
        .get_window(constants::STARTUP_WIN_NAME)
        .expect("Unable to get startup window");

    // Run migrations
    log::info!("Running migrations");
    let _ = window.emit(
        ClientEvent::StartupProgress.as_ref(),
        "Running migrations...",
    );

    if let Err(err) = Migrator::run_migrations().await {
        // Ruh-oh something went wrong
        sentry::capture_error(&err);
        log::error!("Unable to migrate database - {}", err.to_string());

        // Let users know something has gone wrong.
        alert(&window, "Migration Failure", &err.to_string());
        app_handle.exit(0);
    }

    // Wait for the server to boot up
    log::info!("Waiting for server backend");
    let _ = window.emit(
        ClientEvent::StartupProgress.as_ref(),
        "Waiting for backend...",
    );

    let rpc = rpc::RpcClient::new().await;
    app_handle.manage(Arc::new(Mutex::new(rpc)));

    let _ = window.hide();
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
