use num_format::{Locale, ToFormattedString};
use tauri::{
    api::dialog::blocking::message,
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, RunEvent, Wry,
};
use tokio::sync::Mutex;
use tokio::time::{self, Duration};

use migration::Migrator;
use shared::response;
use shared::response::AppStatus;
use spyglass_rpc::RpcClient;
const TRAY_UPDATE_INTERVAL_S: u64 = 60;

use crate::{
    constants,
    menu::MenuID,
    rpc::{self, RpcMutex},
};

pub struct StartupProgressText(std::sync::Mutex<String>);
impl StartupProgressText {
    pub fn set(&self, new_value: &str) {
        if let Ok(mut value) = self.0.lock() {
            *value = new_value.to_owned();
        }
    }
}

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("tauri-plugin-startup")
        .invoke_handler(tauri::generate_handler![get_startup_progress])
        .on_event(|app_handle, event| {
            if let RunEvent::Ready = event {
                app_handle.manage(StartupProgressText(std::sync::Mutex::new(
                    "Running startup tasks...".to_string(),
                )));

                // Don't block the main thread
                tauri::async_runtime::spawn(run_and_check_backend(app_handle.clone()));

                // Keep system tray stats updated
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let mut interval = time::interval(Duration::from_secs(TRAY_UPDATE_INTERVAL_S));
                    loop {
                        update_tray_menu(&app_handle).await;
                        interval.tick().await;
                    }
                });
            }
        })
        .build()
}

#[tauri::command]
async fn get_startup_progress(window: tauri::Window) -> Result<String, String> {
    let app_handle = window.app_handle();
    if let Some(mutex) = app_handle.try_state::<StartupProgressText>() {
        if let Ok(progress) = mutex.0.lock() {
            return Ok(progress.to_string());
        }
    }

    Ok("Running startup tasks...".to_string())
}

async fn run_and_check_backend(app_handle: AppHandle) {
    log::info!("Running startup tasks");
    let progress = app_handle.state::<StartupProgressText>();
    let window = app_handle
        .get_window(constants::STARTUP_WIN_NAME)
        .expect("Unable to get startup window");

    // Run migrations
    log::info!("Running migrations");
    progress.set("Running migrations...");
    if let Err(err) = Migrator::run_migrations().await {
        // Ruh-oh something went wrong
        sentry::capture_error(&err);
        log::error!("Unable to migrate database - {}", err.to_string());
        progress.set(&format!("Unable to migrate database: {}", &err.to_string()));

        // Let users know something has gone wrong.
        message(
            Some(&window),
            "Migration Failure",
            format!(
                "Migration error: {}\nPlease file a bug report!\nThe application will exit now.",
                &err.to_string()
            ),
        );

        app_handle.exit(0);
    }

    // Wait for the server to boot up
    log::info!("Waiting for server backend");
    progress.set("Waiting for backend...");
    let rpc = rpc::RpcClient::new().await;
    app_handle.manage(RpcMutex::new(Mutex::new(rpc)));

    // Will cancel and clear any interval checks in the client
    progress.set("DONE");
    let _ = window.hide();
}

async fn update_tray_menu(app: &AppHandle) {
    if let Some(rpc) = app.try_state::<RpcMutex>() {
        let rpc = rpc.inner();
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
}

async fn app_status(rpc: &rpc::RpcMutex) -> Option<response::AppStatus> {
    let rpc = rpc.lock().await;
    match rpc.client.app_status().await {
        Ok(resp) => Some(resp),
        Err(err) => {
            log::error!("Error sending RPC: {}", err);
            None
        }
    }
}
