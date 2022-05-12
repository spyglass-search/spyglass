#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use jsonrpc_core::Value;
use num_format::{Locale, ToFormattedString};
use rpc::RpcMutex;
use tauri::{
    AppHandle, GlobalShortcutManager, Manager, SystemTray, SystemTrayEvent, WindowBuilder,
    WindowUrl
};
use tokio::sync::Mutex;
use tokio::time;
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

use shared::config::Config;
use shared::response;
use shared::response::AppStatus;

mod cmd;
mod constants;
mod menu;
mod rpc;
mod window;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new();

    let file_appender = tracing_appender::rolling::daily(config.logs_dir(), "client.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with(
            fmt::Layer::new()
                .with_thread_names(true)
                .with_writer(io::stdout),
        )
        .with(fmt::Layer::new().with_ansi(false).with_writer(non_blocking));

    tracing::subscriber::set_global_default(subscriber).expect("Unable to set a global subscriber");
    LogTracer::init()?;

    let ctx = tauri::generate_context!();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            cmd::escape,
            cmd::open_result,
            cmd::search_docs,
            cmd::search_lenses,
            cmd::resize_window
        ])
        .menu(menu::get_app_menu())
        .system_tray(SystemTray::new().with_menu(menu::get_tray_menu(&config)))
        .setup(move |app| {
            WindowBuilder::new(app, "stats", WindowUrl::App("/stats".into()))
                .title("Status")
                .build()?;

            // hide from dock (also hides menu bar)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Start up backend (only in release mode)
            #[cfg(not(debug_assertions))]
            rpc::check_and_start_backend();

            let window = app.get_window("main").unwrap();
            let _ = window.set_skip_taskbar(true);

            // Wait for the server to boot up
            let rpc = tauri::async_runtime::block_on(rpc::RpcClient::new());
            app.manage(Arc::new(Mutex::new(rpc)));

            // Load user settings
            app.manage(config.clone());

            // Register global shortcut
            let mut shortcuts = app.global_shortcut_manager();
            if !shortcuts
                .is_registered(&config.user_settings.shortcut)
                .unwrap()
            {
                log::info!("Registering {} as shortcut", &config.user_settings.shortcut);
                let window = window.clone();
                shortcuts
                    .register(&config.user_settings.shortcut, move || {
                        if window.is_visible().unwrap() {
                            window::hide_window(&window);
                        } else {
                            window::show_window(&window);
                        }
                    })
                    .unwrap();
            }

            // Center window horizontally in the current screen
            window::center_window(&window);

            // Keep system tray stats updated
            let app_handle = app.app_handle();
            tauri::async_runtime::spawn(async move {
                let mut interval = time::interval(Duration::from_secs(10));
                loop {
                    update_tray_menu(&app_handle).await;
                    interval.tick().await;
                }
            });

            Ok(())
        })
        .on_window_event(|event| {
            if let tauri::WindowEvent::Focused(is_focused) = event.event() {
                if !is_focused {
                    let handle = event.window();
                    window::hide_window(handle);
                }
            }
        })
        .on_system_tray_event(|app, event| {
            if let SystemTrayEvent::MenuItemClick { id, .. } = event {
                let config = app.state::<Config>();
                let item_handle = app.tray_handle().get_item(&id);
                let window = app.get_window("main").unwrap();

                match id.as_str() {
                    menu::CRAWL_STATUS_MENU_ITEM => {
                        let rpc = app.state::<RpcMutex>().inner();
                        let is_paused = tauri::async_runtime::block_on(pause_crawler(rpc));
                        let new_label = if is_paused {
                            "▶️ Resume indexing"
                        } else {
                            "⏸ Pause indexing"
                        };

                        item_handle.set_title(new_label).unwrap();
                    }
                    menu::OPEN_LENSES_FOLDER => open_folder(config.lenses_dir()),
                    menu::OPEN_LOGS_FOLDER => open_folder(config.logs_dir()),
                    menu::OPEN_SETTINGS_FOLDER => open_folder(Config::prefs_dir()),
                    menu::SHOW_SEARCHBAR => {
                        if !window.is_visible().unwrap() {
                            window::show_window(&window);
                        }
                    }
                    menu::QUIT_MENU_ITEM => app.exit(0),
                    menu::DEV_SHOW_CONSOLE => window.open_devtools(),
                    menu::JOIN_DISCORD => open::that(constants::DISCORD_JOIN_URL).unwrap(),
                    _ => {}
                }
            }
        })
        .run(ctx)
        .expect("error while running tauri application");

    Ok(())
}

async fn app_status(rpc: &rpc::RpcMutex) -> Option<response::AppStatus> {
    let mut rpc = rpc.lock().await;
    match rpc
        .client
        .call_method::<Value, response::AppStatus>("app_stats", "", Value::Null)
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

async fn pause_crawler(rpc: &rpc::RpcMutex) -> bool {
    let mut rpc = rpc.lock().await;
    match rpc
        .client
        .call_method::<Value, response::AppStatus>("toggle_pause", "", Value::Null)
        .await
    {
        Ok(resp) => resp.is_paused,
        Err(err) => {
            log::error!("Error sending RPC: {}", err);
            rpc.reconnect().await;
            false
        }
    }
}

fn open_folder(folder: PathBuf) {
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open")
        .arg(folder)
        .spawn()
        .unwrap();

    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg(folder)
        .spawn()
        .unwrap();

    #[cfg(target_os = "windows")]
    std::process::Command::new("explorer")
        .arg(folder)
        .spawn()
        .unwrap();
}

async fn update_tray_menu(app: &AppHandle) {
    let rpc = app.state::<RpcMutex>().inner();
    let app_status: Option<AppStatus> = app_status(rpc).await;
    let handle = app.tray_handle();

    if let Some(app_status) = app_status {
        handle
            .get_item(menu::CRAWL_STATUS_MENU_ITEM)
            .set_title(if app_status.is_paused {
                "▶️ Resume indexing"
            } else {
                "⏸ Pause indexing"
            })
            .unwrap();

        handle
            .get_item(menu::NUM_DOCS_MENU_ITEM)
            .set_title(format!(
                "{} documents indexed",
                app_status.num_docs.to_formatted_string(&Locale::en)
            ))
            .unwrap();

        handle
            .get_item(menu::NUM_QUEUED_MENU_ITEM)
            .set_title(format!(
                "{} in queue",
                app_status.num_queued.to_formatted_string(&Locale::en)
            ))
            .unwrap();

        handle
            .get_item(menu::NUM_IN_PROGRESS_MENU_ITEM)
            .set_title(format!(
                "{} crawling",
                app_status.num_in_progress.to_formatted_string(&Locale::en)
            ))
            .unwrap();
    }
}
