#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use jsonrpc_core::Value;
use num_format::{Locale, ToFormattedString};
use std::path::PathBuf;
use tauri::{AppHandle, GlobalShortcutManager, Manager, SystemTray, SystemTrayEvent};

use shared::config::Config;
use shared::response;

mod cmd;
mod constants;
mod menu;
mod rpc;
mod window;

fn main() {
    let file_appender = tracing_appender::rolling::daily(Config::logs_dir(), "client.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_thread_names(true)
        .with_writer(non_blocking)
        .init();

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
        .setup(|app| {
            // hide from dock (also hides menu bar)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Only show in dev/debug mode.
            #[cfg(debug_assertions)]
            app.get_window("main").unwrap().open_devtools();

            let window = app.get_window("main").unwrap();

            // Start up backend (only in release mode)
            #[cfg(not(debug_assertions))]
            rpc::check_and_start_backend();

            // Wait for the server to boot up
            app.manage(tauri::async_runtime::block_on(rpc::RpcClient::new()));

            // Load user settings
            let config = Config::new();
            app.manage(config.clone());

            // Register global shortcut
            let mut shortcuts = app.global_shortcut_manager();
            if !shortcuts.is_registered(&config.user_settings.shortcut).unwrap() {
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

            Ok(())
        })
        .system_tray(SystemTray::new().with_menu(menu::get_tray_menu()))
        .on_window_event(|event| {
            if let tauri::WindowEvent::Focused(is_focused) = event.event() {
                if !is_focused {
                    let handle = event.window();
                    window::hide_window(handle);
                }
            }
        })
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::LeftClick { .. } => update_tray_menu(app),
            SystemTrayEvent::RightClick { .. } => update_tray_menu(app),
            SystemTrayEvent::MenuItemClick { id, .. } => {
                let item_handle = app.tray_handle().get_item(&id);
                match id.as_str() {
                    menu::CRAWL_STATUS_MENU_ITEM => {
                        let rpc = app.state::<rpc::RpcClient>().inner();

                        let is_paused = tauri::async_runtime::block_on(pause_crawler(rpc));
                        let new_label = if is_paused {
                            "▶️ Resume indexing"
                        } else {
                            "⏸ Pause indexing"
                        };

                        item_handle.set_title(new_label).unwrap();
                    }
                    menu::OPEN_LENSES_FOLDER => {
                        open_folder(Config::lenses_dir());
                    }
                    menu::OPEN_SETTINGS_FOLDER => {
                        open_folder(Config::prefs_dir());
                    }
                    menu::TOGGLE_MENU_ITEM => {
                        let window = app.get_window("main").unwrap();
                        let new_title = if window.is_visible().unwrap() {
                            window::hide_window(&window);
                            "Show"
                        } else {
                            window::show_window(&window);
                            "Hide"
                        };
                        item_handle.set_title(new_title).unwrap();
                    }
                    menu::QUIT_MENU_ITEM => {
                        app.exit(0);
                    }
                    _ => {}
                }
            }
            _ => {}
        })
        .run(ctx)
        .expect("error while running tauri application");
}

async fn app_status(rpc: &rpc::RpcClient) -> Option<response::AppStatus> {
    match rpc
        .client
        .call_method::<Value, response::AppStatus>("app_stats", "", Value::Null)
        .await
    {
        Ok(resp) => Some(resp),
        Err(err) => {
            log::error!("{}", err);
            None
        }
    }
}

async fn pause_crawler(rpc: &rpc::RpcClient) -> bool {
    match rpc
        .client
        .call_method::<Value, response::AppStatus>("toggle_pause", "", Value::Null)
        .await
    {
        Ok(resp) => resp.is_paused,
        Err(err) => {
            log::error!("{}", err);
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

fn update_tray_menu(app: &AppHandle) {
    let rpc = app.state::<rpc::RpcClient>().inner();

    let app_status = tauri::async_runtime::block_on(app_status(rpc));
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
    }
}
