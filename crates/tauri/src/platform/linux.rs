use crate::window;
use shared::event::ClientEvent;
use tauri::Window;

pub fn show_search_bar(window: &Window) {
    let _ = window.unminimize();
    window::center_search_bar(window);
    let _ = window.set_focus();
    let _ = window.set_always_on_top(true);

    let _ = window.emit(ClientEvent::FocusWindow.as_ref(), true);
}

pub fn hide_search_bar(window: &Window) {
    let _ = window.minimize();
    let _ = window.emit(ClientEvent::ClearSearch.as_ref(), true);
}
