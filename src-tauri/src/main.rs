#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tauri::{
    CustomMenuItem, Menu, MenuItem, Submenu, SystemTray, SystemTrayMenu, SystemTrayMenuItem,
};

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
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let hide = CustomMenuItem::new("hide".to_string(), "Hide");
    let tray_menu = SystemTrayMenu::new()
        .add_item(quit)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(hide);

    let tray = SystemTray::new().with_menu(tray_menu);

    let ctx = tauri::generate_context!();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![search])
        .menu(
            Menu::new().add_submenu(Submenu::new(
                &ctx.package_info().name,
                Menu::new()
                    .add_native_item(MenuItem::Hide)
                    .add_native_item(MenuItem::Separator)
                    .add_native_item(MenuItem::Quit),
            )),
        )
        .system_tray(tray)
        .run(ctx)
        .expect("error while running tauri application");
}

#[tauri::command]
async fn search(query: &str) -> Result<Vec<SearchResult>, String> {
    let mut map = HashMap::new();
    map.insert("term", query);

    let res: SearchResults = reqwest::Client::new()
        .post("http://localhost:7777/api/search")
        .json(&map)
        .send()
        .await.unwrap()
        .json()
        .await.unwrap();

    println!("{:?}", res);

    Ok(res.results)
}
