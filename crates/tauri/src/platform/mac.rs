use shared::event::ClientEvent;
use tauri::Window;
use url::Url;

use crate::window;

pub fn show_search_bar(window: &Window) {
    let _ = window.show();
    window::center_search_bar(window);
    let _ = window.set_always_on_top(true);
    let _ = window.set_focus();
}

pub fn hide_search_bar(window: &Window) {
    let _ = window.hide();
    let _ = window.emit(ClientEvent::ClearSearch.as_ref(), true);
}

pub fn os_open(url: &Url) -> anyhow::Result<()> {
    match open::that(url.to_string()) {
        Ok(_) => Ok(()),
        Err(err) => Err(anyhow::anyhow!(err.to_string())),
    }
}
