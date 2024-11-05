use shared::event::ClientEvent;
use tauri::{Emitter, Manager, WebviewWindow};
use url::Url;

use crate::window;

pub fn is_visible(window: &WebviewWindow) -> bool {
    window.is_visible().unwrap_or_default()
}

pub fn show_search_bar(window: &WebviewWindow) {
    let _ = tauri::AppHandle::show(window.app_handle());
    let _ = window.set_focus();
    window::center_search_bar(window);
}

pub fn hide_search_bar(window: &WebviewWindow) {
    let _ = tauri::AppHandle::hide(window.app_handle());
    let _ = window.emit(ClientEvent::ClearSearch.as_ref(), true);
}

pub fn os_open(url: &Url, application: Option<String>) -> anyhow::Result<()> {
    let open_url = if url.scheme() == "file" {
        use shared::url_to_file_path;
        let file_path = url.to_file_path().unwrap_or_else(|_| url.path().into());
        url_to_file_path(&file_path.display().to_string(), false)
    } else {
        url.to_string()
    };

    if let Some(application) = application {
        return match open::with(open_url, application) {
            Ok(_) => Ok(()),
            Err(err) => Err(anyhow::anyhow!(err.to_string())),
        };
    }

    match open::that(open_url) {
        Ok(_) => Ok(()),
        Err(err) => Err(anyhow::anyhow!(err.to_string())),
    }
}
