use tauri::{AppHandle, LogicalSize, Manager, Size, Window, WindowBuilder, WindowUrl};

use crate::constants;

pub fn center_window(window: &Window) {
    if let Some(monitor) = window.primary_monitor().unwrap() {
        let size = monitor.size();
        let scale = monitor.scale_factor();

        let middle = (size.width as f64 / (scale * 2.0)) - (constants::INPUT_WIDTH / 2.0);

        window
            .set_position(tauri::Position::Logical(tauri::LogicalPosition {
                x: middle,
                y: constants::INPUT_Y,
            }))
            .unwrap();
    }
}

pub fn hide_window(window: &Window) {
    window.hide().unwrap();
    window.emit("clear_search", true).unwrap();
}

pub async fn resize_window(window: &Window, height: f64) {
    window
        .set_size(Size::Logical(LogicalSize {
            width: constants::INPUT_WIDTH,
            height,
        }))
        .unwrap();
}

pub fn show_window(window: &Window) {
    window.emit("focus_window", true).unwrap();
    window.show().unwrap();
    window.set_focus().unwrap();
    center_window(window);
}

pub fn show_crawl_stats_window(app: &AppHandle) -> Window {
    if let Some(window) = app.get_window(constants::STATS_WIN_NAME) {
        let _ = window.show();
        let _ = window.set_focus();
        return window;
    }

    WindowBuilder::new(
        app,
        constants::STATS_WIN_NAME,
        WindowUrl::App("/stats".into()),
    )
    .title("Status")
    .build()
    .unwrap()
}

pub fn show_lens_manager_window(app: &AppHandle) -> Window {
    if let Some(window) = app.get_window(constants::LENS_MANAGER_WIN_NAME) {
        let _ = window.show();
        let _ = window.set_focus();
        return window;
    }

    WindowBuilder::new(
        app,
        constants::LENS_MANAGER_WIN_NAME,
        WindowUrl::App("/settings/lens".into()),
    )
    .title("Lens Manager")
    .build()
    .unwrap()
}
