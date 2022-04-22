#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use jsonrpc_core::Value;
use num_format::{Locale, ToFormattedString};
use tauri::api::process::Command;
use tauri::{
    GlobalShortcutManager, LogicalSize, Manager, Size, State, SystemTray, SystemTrayEvent, Window,
};

use shared::config::Config;
use shared::{request, response};

mod menu;
mod rpc;

const INPUT_WIDTH: f64 = 640.0;
const INPUT_HEIGHT: f64 = 80.0;
const INPUT_Y: f64 = 128.0;

const SHORTCUT: &str = "CmdOrCtrl+Shift+/";

fn _center_window(window: &Window) {
    if let Some(monitor) = window.current_monitor().unwrap() {
        let size = monitor.size();
        let scale = monitor.scale_factor();

        let middle = (size.width as f64 / (scale * 2.0)) - (INPUT_WIDTH / 2.0);

        window
            .set_position(tauri::Position::Logical(tauri::LogicalPosition {
                x: middle,
                y: INPUT_Y,
            }))
            .unwrap();
    }
}

fn _hide_window(window: &Window) {
    window.hide().unwrap();
    window.emit("clear_search", true).unwrap();
}

fn _show_window(window: &Window) {
    window.show().unwrap();
    window.set_focus().unwrap();
    resize_window(window.clone(), INPUT_HEIGHT);
}

#[allow(dead_code)]
fn check_and_start_backend() {
    let _ = Command::new_sidecar("spyglass-server")
        .expect("failed to create `spyglass-server` binary command")
        .spawn()
        .expect("Failed to spawn sidecar");
}

fn main() {
    let ctx = tauri::generate_context!();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            escape,
            open_result,
            search_docs,
            search_lenses,
            resize_window
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
            check_and_start_backend();

            // Wait for the server to boot up
            app.manage(tauri::async_runtime::block_on(rpc::RpcClient::new()));

            // Register global shortcut
            let mut shortcuts = app.global_shortcut_manager();
            if !shortcuts.is_registered(SHORTCUT).unwrap() {
                let window = window.clone();
                shortcuts
                    .register(SHORTCUT, move || {
                        if window.is_visible().unwrap() {
                            _hide_window(&window);
                        } else {
                            _show_window(&window);
                        }
                    })
                    .unwrap();
            }

            // Center window horizontally in the current screen
            _center_window(&window);

            Ok(())
        })
        .system_tray(SystemTray::new().with_menu(menu::get_tray_menu()))
        .on_window_event(|event| {
            if let tauri::WindowEvent::Focused(is_focused) = event.event() {
                if !is_focused {
                    let handle = event.window();
                    _hide_window(handle);
                }
            }
        })
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::LeftClick { .. } => {
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
                        std::process::Command::new("open")
                            .arg(Config::lenses_dir())
                            .spawn()
                            .unwrap();
                    }
                    menu::OPEN_SETTINGS_FOLDER => {
                        std::process::Command::new("open")
                            .arg(Config::prefs_dir())
                            .spawn()
                            .unwrap();
                    }
                    menu::TOGGLE_MENU_ITEM => {
                        let window = app.get_window("main").unwrap();
                        let new_title = if window.is_visible().unwrap() {
                            _hide_window(&window);
                            "Show"
                        } else {
                            _show_window(&window);
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

#[tauri::command]
async fn escape(window: tauri::Window) -> Result<(), String> {
    _hide_window(&window);
    Ok(())
}

#[tauri::command]
async fn open_result(_: tauri::Window, url: &str) -> Result<(), String> {
    open::that(url).unwrap();
    Ok(())
}

#[tauri::command]
fn resize_window(window: tauri::Window, height: f64) {
    window
        .set_size(Size::Logical(LogicalSize {
            width: INPUT_WIDTH,
            height,
        }))
        .unwrap();
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

#[tauri::command]
async fn search_docs<'r>(
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
async fn search_lenses<'r>(
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
