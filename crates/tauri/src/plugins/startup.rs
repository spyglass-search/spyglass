use tauri::{
    api::dialog::blocking::message,
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, RunEvent, Wry,
};
use tokio::sync::{broadcast, Mutex};

use migration::Migrator;
use shared::config::Config;

use crate::window::show_wizard_window;
use crate::{rpc::SpyglassServerClient, window::get_searchbar};

use crate::{rpc::RpcMutex, window, AppEvent};
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
            match event {
                RunEvent::Ready => {
                    app_handle.manage(StartupProgressText(std::sync::Mutex::new(
                        "Running startup tasks...".to_string(),
                    )));

                    // Don't block the main thread
                    tauri::async_runtime::spawn(run_and_check_backend(app_handle.clone()));
                }
                RunEvent::Exit => {
                    let app_handle = app_handle.clone();
                    if let Some(rpc) = app_handle.try_state::<RpcMutex>() {
                        tauri::async_runtime::block_on(async move {
                            let rpc = rpc.lock().await;
                            if let Some(sidecar) = &rpc.sidecar_handle {
                                sidecar.abort();
                            }
                        });
                    }
                }
                _ => {}
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
    let window = window::show_startup_window(&app_handle);

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

    let config = app_handle.state::<Config>();
    let rpc = SpyglassServerClient::new(&config, &app_handle).await;
    let rpc_mutex = RpcMutex::new(Mutex::new(rpc));
    app_handle.manage(rpc_mutex.clone());

    // Let plugins know the server is connected.
    let app_events = app_handle.state::<broadcast::Sender<AppEvent>>();
    let _ = app_events.send(AppEvent::BackendConnected);

    // Watch and restart backend if it goes down
    tauri::async_runtime::spawn(SpyglassServerClient::daemon_eyes(
        rpc_mutex,
        app_events.subscribe(),
    ));

    // Will cancel and clear any interval checks in the client
    progress.set("DONE");
    let _ = window.hide();

    // Run wizard on first run
    if !config.user_settings.run_wizard {
        show_wizard_window(&window.app_handle());
    } else {
        let sbar = get_searchbar(&app_handle);
        window::show_search_bar(&sbar);
    }
}
