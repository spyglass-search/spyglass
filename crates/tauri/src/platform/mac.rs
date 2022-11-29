use tauri::Window;

use crate::window;
use shared::event::ClientEvent;

pub fn show_search_bar(window: &Window) {
    let _ = window.show();
    window::center_search_bar(window);
    let _ = window.set_always_on_top(true);
    let _ = window.set_focus();

    let _ = window.emit(ClientEvent::FocusWindow.as_ref(), true);
}

pub fn hide_search_bar(window: &Window) {
    let _ = window.hide();
    let _ = window.emit(ClientEvent::ClearSearch.as_ref(), true);
}
