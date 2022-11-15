pub fn show_search_bar(window: &Window) {
    let _ = window.unminimize();
    window::center_search_bar(window);
    let _ = window.set_focus();

    let _ = window.emit(ClientEvent::FocusWindow.as_ref(), true);
}

pub fn hide_search_bar(window: &Window) {
    let _ = window.minimize();
    let _ = window.emit(ClientEvent::ClearSearch.as_ref(), true);
}