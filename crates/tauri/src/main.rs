#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use std::collections::HashMap;

use num_format::{Locale, ToFormattedString};
use serde::Serialize;
use tauri::{GlobalShortcutManager, LogicalSize, Manager, Size, SystemTray, SystemTrayEvent};

use shared::response::{AppStatus, SearchResult, SearchResults};
mod menu;

const INPUT_WIDTH: f64 = 640.0;
const INPUT_HEIGHT: f64 = 80.0;
const INPUT_Y: f64 = 128.0;

const SHORTCUT: &str = "CmdOrCtrl+Shift+/";
const API_ENDPOINT: &str = "http://localhost:7777";

fn main() {
    let ctx = tauri::generate_context!();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![escape, open_result, search, resize_window])
        .menu(menu::get_app_menu())
        .setup(|app| {
            // hide from dock (also hides menu bar)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.get_window("main").unwrap().open_devtools();

            // Register global shortcut
            let mut shortcuts = app.global_shortcut_manager();
            if !shortcuts.is_registered(SHORTCUT).unwrap() {
                let handle = app.get_window("main").unwrap();
                shortcuts
                    .register(SHORTCUT, move || {
                        if handle.is_visible().unwrap() {
                            handle.hide().unwrap();
                        } else {
                            handle.show().unwrap();
                            handle
                                .set_size(Size::Logical(LogicalSize {
                                    width: INPUT_WIDTH,
                                    height: INPUT_HEIGHT,
                                }))
                                .unwrap();
                            handle.set_focus().unwrap();
                        }
                    })
                    .unwrap();
            }

            // Center window horizontally in the current screen
            let window = app.get_window("main").unwrap();
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
            Ok(())
        })
        .system_tray(SystemTray::new().with_menu(menu::get_tray_menu()))
        .on_window_event(|event| {
            if let tauri::WindowEvent::Focused(is_focused) = event.event() {
                if !is_focused {
                    event.window().hide().unwrap();
                    event.window().emit("clear_search", true).unwrap();
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
                            window.hide().unwrap();
                            "Show"
                        } else {
                            window.show().unwrap();
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
    window.hide().unwrap();
    Ok(())
}

#[tauri::command]
async fn open_result(_: tauri::Window, url: &str) -> Result<(), String> {
    open::that(url).unwrap();
    Ok(())
}

fn app_status() -> AppStatus {
    let client = reqwest::blocking::Client::new();

    let res: AppStatus = client
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

    let res: AppStatus = client
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

#[derive(Serialize)]
struct SearchRequest {
    lenses: Vec<String>,
    query: String,
}

#[tauri::command]
async fn search(_: tauri::Window, lenses: Vec<String>, query: &str) -> Result<Vec<SearchResult>, String> {
    let data = SearchRequest {
        lenses,
        query: query.to_string(),
    };

    let res: SearchResults = reqwest::Client::new()
        // TODO: make this configurable
        .post(format!("{}/api/search", API_ENDPOINT))
        .json(&data)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let results: Vec<SearchResult> = res.results.to_vec();
    Ok(results)
}
