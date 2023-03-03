use crate::window;
use shared::event::ClientEvent;
use tauri::api::process::current_binary;
use tauri::{Env, Window};
use url::Url;

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

pub fn os_open(url: &Url) -> anyhow::Result<()> {
    let binary_path = current_binary(&Env::default())?;
    let parent = if let Some(parent) = binary_path.parent() {
        parent.to_path_buf()
    } else {
        binary_path
    };

    match tauri::api::process::Command::new("xdg-open")
        .args(vec![url.to_string()])
        .current_dir(parent)
        .output() {
        Ok(_) => Ok(()),
        Err(err) => Err(anyhow::anyhow!(err.to_string()))
    }
}
