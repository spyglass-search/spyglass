use std::str::FromStr;
use tauri::Manager;

use crate::constants::WindowLocation;

#[tauri::command]
pub async fn escape(window: tauri::WebviewWindow) -> Result<(), String> {
    crate::window::hide_search_bar(&window);
    Ok(())
}

#[tauri::command]
pub async fn resize_window(window: tauri::WebviewWindow, height: f64) {
    crate::window::resize_window(&window, height).await;
}

#[tauri::command]
pub async fn navigate(win: tauri::Window, page: String) -> Result<(), String> {
    if let Ok(tab_loc) = WindowLocation::from_str(&page) {
        crate::window::navigate_to_tab(win.app_handle(), &tab_loc);
    }

    Ok(())
}

#[tauri::command]
pub async fn open_big_mode(win: tauri::WebviewWindow) -> Result<(), String> {
    crate::window::show_bigmode(win.app_handle());
    Ok(())
}
