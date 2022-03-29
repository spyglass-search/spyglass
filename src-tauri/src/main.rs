#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{GlobalShortcutManager, LogicalSize, Manager, Size, SystemTray, SystemTrayEvent};
mod menu;

const INPUT_WIDTH: f64 = 640.0;
const INPUT_HEIGHT: f64 = 80.0;
const INPUT_Y: f64 = 128.0;

const RESULT_HEIGHT: f64 = 126.0;

const SHORTCUT: &str = "CmdOrCtrl+Shift+/";

#[derive(Debug, Deserialize, Serialize)]
pub struct AppStatus {
    pub is_paused: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchMeta {
    pub query: String,
    pub num_docs: u64,
    pub wall_time_ms: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchResult {
    pub title: String,
    pub description: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchResults {
    pub results: Vec<SearchResult>,
    pub meta: SearchMeta,
}

fn main() {
    let ctx = tauri::generate_context!();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![escape, open_result, search,])
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
                    event.window().emit("clear_search", 1).unwrap();
                }
            }
        })
        .on_system_tray_event(move |app, event| {
            if let SystemTrayEvent::MenuItemClick { id, .. } = event {
                let item_handle = app.tray_handle().get_item(&id);
                match id.as_str() {
                    "pause" => {
                        let new_label = if pause_crawler() {
                            "Resume indexing"
                        } else {
                            "Pause indexing"
                        };

                        item_handle.set_title(new_label).unwrap();
                    }
                    "toggle" => {
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
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                }
            }
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

fn pause_crawler() -> bool {
    let client = reqwest::blocking::Client::new();
    let mut map = HashMap::new();
    map.insert("toggle_pause", true);

    let res: AppStatus = client
        .post("http://localhost:7777/api/status")
        .json(&map)
        .send()
        .unwrap()
        .json()
        .unwrap();

    res.is_paused
}

#[tauri::command]
async fn search(window: tauri::Window, query: &str) -> Result<Vec<SearchResult>, String> {
    let mut map = HashMap::new();
    map.insert("term", query);

    let res: SearchResults = reqwest::Client::new()
        // TODO: make this configurable
        .post("http://localhost:7777/api/search")
        .json(&map)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let results: Vec<SearchResult> = res.results[0..5].to_vec();

    let num_results = results.len();

    if num_results > 0 {
        window
            .set_size(Size::Logical(LogicalSize {
                width: INPUT_WIDTH,
                height: INPUT_HEIGHT + (num_results.min(results.len()) as f64 * RESULT_HEIGHT),
            }))
            .unwrap();
    } else {
        window
            .set_size(Size::Logical(LogicalSize {
                width: INPUT_WIDTH,
                height: INPUT_HEIGHT,
            }))
            .unwrap();
    }

    Ok(results)
}
