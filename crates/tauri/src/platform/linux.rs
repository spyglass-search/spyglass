use crate::window;
use shared::event::ClientEvent;
use tauri::api::process::current_binary;
use tauri::{Env, Window};
use url::Url;

pub fn is_visible(window: &Window) {
    window.is_visible().unwrap_or_default()
}

pub fn show_search_bar(window: &Window) {
    let _ = window.show();
    let _ = window.unminimize();
    window::center_search_bar(window);
    let _ = window.set_focus();
    let _ = window.set_always_on_top(true);
}

pub fn hide_search_bar(window: &Window) {
    let _ = window.minimize();
    let _ = window.emit(ClientEvent::ClearSearch.as_ref(), true);
}

pub fn os_open(url: &Url, application: Option<String>) -> anyhow::Result<()> {
    let binary_path = current_binary(&Env::default())?;
    let parent = if let Some(parent) = binary_path.parent() {
        parent.to_path_buf()
    } else {
        binary_path
    };

    let open_url = if url.scheme() == "file" {
        use shared::url_to_file_path;
        url_to_file_path(url.path(), false)
    } else {
        url.to_string()
    };

    let app = match &application {
        Some(app) => app.clone(),
        None => String::from("xdg-open"),
    };

    match tauri::api::process::Command::new(app)
        .args(vec![open_url])
        .current_dir(parent)
        .output()
    {
        Ok(_) => Ok(()),
        Err(err) => Err(anyhow::anyhow!(err.to_string())),
    }
}
