use tauri::{LogicalSize, Size, Window};

use crate::{cmd, constants};

pub fn center_window(window: &Window) {
    if let Some(monitor) = window.current_monitor().unwrap() {
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

pub fn resize_window(window: &Window, height: f64) {
    window
        .set_size(Size::Logical(LogicalSize {
            width: constants::INPUT_WIDTH,
            height,
        }))
        .unwrap();
}

pub fn show_window(window: &Window) {
    window.show().unwrap();
    window.set_focus().unwrap();
    cmd::resize_window(window.clone(), constants::INPUT_HEIGHT);
    center_window(window);
}
