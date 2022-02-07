#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use serde::Serialize;
use tauri::{Menu, MenuItem, Submenu};

fn main() {
    let ctx = tauri::generate_context!();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![search])
        .menu(Menu::new().add_submenu(Submenu::new(
            &ctx.package_info().name,
            Menu::new().add_native_item(MenuItem::Quit),
        )))
        .run(ctx)
        .expect("error while running tauri application");
}

#[derive(Debug, Serialize)]
struct SearchResult {
    title: String,
    description: String,
    url: String,
}

#[tauri::command]
async fn search(query: &str) -> Result<Vec<SearchResult>, String> {
    let mut test = Vec::new();
    test.push(SearchResult {
        title: format!("query: {}", query),
        description: "lorem ipsum".to_string(),
        url: "https://google.com".to_string(),
    });
    test.push(SearchResult {
        title: "Title 2".to_string(),
        description: "lorem ipsum".to_string(),
        url: "https://google.com".to_string(),
    });
    test.push(SearchResult {
        title: "Title 3".to_string(),
        description: "lorem ipsum".to_string(),
        url: "https://google.com".to_string(),
    });

    Ok(test)
}
