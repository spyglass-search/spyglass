use crate::window;
use shared::event::ClientEvent;
use tauri::Window;
use url::Url;

pub fn show_search_bar(window: &Window) {
    if let Ok(false) = window.is_visible() {
        let _ = window.show();
    }

    let _ = window.unminimize();
    window::center_search_bar(window);
    let _ = window.set_focus();
}

pub fn hide_search_bar(window: &Window) {
    let _ = window.minimize();
    let _ = window.emit(ClientEvent::ClearSearch.as_ref(), true);
}

pub fn os_open(url: &Url, application: Option<String>) -> anyhow::Result<()> {
    let open_url = if url.scheme() == "file" {
        use shared::url_to_file_path;
        url_to_file_path(url.path(), true)
    } else {
        url.to_string()
    };

    if let Some(application) = application {
        match open::with(open_url, application) {
            Ok(_) => Ok(()),
            Err(err) => Err(anyhow::anyhow!(err.to_string())),
        }
    } else {
        match open::that(open_url) {
            Ok(_) => Ok(()),
            Err(err) => Err(anyhow::anyhow!(err.to_string())),
        }
    }
}
