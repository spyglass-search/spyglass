#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{LogicalSize, GlobalShortcutManager, Manager, Size, SystemTray, SystemTrayEvent};

mod menu;

const INPUT_WIDTH: f64 = 640.0;
const INPUT_HEIGHT: f64 = 96.0;
const INPUT_Y: f64 = 128.0;

const RESULT_HEIGHT: f64 = 96.0;


#[derive(Debug, Deserialize, Serialize)]
pub struct SearchMeta {
    pub query: String,
    pub num_docs: u64,
    pub wall_time_ms: u64,
}

#[derive(Debug, Deserialize, Serialize)]
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
        .invoke_handler(tauri::generate_handler![search])
        .menu(menu::get_app_menu())
        .setup(|app| {
            let window = app.get_window("main").unwrap();
            // Center horizontally in the current screen
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
        .on_system_tray_event(move |app, event| {
            if let SystemTrayEvent::MenuItemClick { id, .. } = event {
                let item_handle = app.tray_handle().get_item(&id);
                match id.as_str() {
                    "quit" => {
                        app.exit(0);
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
                    _ => {}
                }
            }
        })
        .run(ctx)
        .expect("error while running tauri application");
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

    println!("search: {:?}", res);
    let num_results = res.results.len();

    if num_results > 0 {
        window
            .set_size(Size::Logical(LogicalSize {
                width: INPUT_WIDTH,
                height: INPUT_HEIGHT + (num_results as f64 * RESULT_HEIGHT),
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

    Ok(res.results)
}
