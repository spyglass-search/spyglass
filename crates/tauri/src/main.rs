#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use std::collections::HashMap;

use num_format::{Locale, ToFormattedString};
use tauri::{
    GlobalShortcutManager, LogicalSize, Manager, Size, SystemTray, SystemTrayEvent, Window,
};

use shared::{request, response};

mod menu;

const INPUT_WIDTH: f64 = 640.0;
const INPUT_HEIGHT: f64 = 80.0;
const INPUT_Y: f64 = 128.0;

const SHORTCUT: &str = "CmdOrCtrl+Shift+/";
const API_ENDPOINT: &str = "http://localhost:7777";

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

            app.get_window("main").unwrap().open_devtools();
            let window = app.get_window("main").unwrap();

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
        .on_system_tray_event(move |app, event| match event {
            SystemTrayEvent::LeftClick { .. } => {
                let app_status = app_status();
                let handle = app.tray_handle();

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
            SystemTrayEvent::MenuItemClick { id, .. } => {
                let item_handle = app.tray_handle().get_item(&id);
                match id.as_str() {
                    menu::CRAWL_STATUS_MENU_ITEM => {
                        let new_label = if pause_crawler() {
                            "▶️ Resume indexing"
                        } else {
                            "⏸ Pause indexing"
                        };

                        item_handle.set_title(new_label).unwrap();
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

fn app_status() -> response::AppStatus {
    let client = reqwest::blocking::Client::new();

    let res: response::AppStatus = client
        .get("http://localhost:7777/api/status")
        .send()
        .unwrap()
        .json()
        .unwrap();

    res
}

fn pause_crawler() -> bool {
    let client = reqwest::blocking::Client::new();
    let mut map = HashMap::new();
    map.insert("toggle_pause", true);

    let res: response::AppStatus = client
        .post(format!("{}/api/status", API_ENDPOINT))
        .json(&map)
        .send()
        .unwrap()
        .json()
        .unwrap();

    res.is_paused
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

#[tauri::command]
async fn search_docs(
    _: tauri::Window,
    lenses: Vec<String>,
    query: &str,
) -> Result<Vec<response::SearchResult>, String> {
    let data = request::SearchParam { lenses, query };

    let res: response::SearchResults = reqwest::Client::new()
        // TODO: make this configurable
        .post(format!("{}/api/search", API_ENDPOINT))
        .json(&data)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let results: Vec<response::SearchResult> = res.results.to_vec();
    Ok(results)
}

#[tauri::command]
async fn search_lenses(_: tauri::Window, query: &str) -> Result<Vec<response::LensResult>, String> {
    let data = request::SearchLensesParam { query };

    let res: response::SearchLensesResp = reqwest::Client::new()
        // TODO: make this configurable
        .post(format!("{}/api/lenses", API_ENDPOINT))
        .json(&data)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let results: Vec<response::LensResult> = res.results.to_vec();
    Ok(results)
}
