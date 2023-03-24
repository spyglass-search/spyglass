use crate::window;
use shared::event::ClientEvent;
use tauri::Window;
use url::Url;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::GetWindowPlacement;
use windows::Win32::UI::WindowsAndMessaging::{SHOW_WINDOW_CMD, WINDOWPLACEMENT};

pub fn is_visible(window: &Window) -> bool {
    if let Ok(handle) = window.hwnd() {
        let mut lpwndpl = WINDOWPLACEMENT::default();
        unsafe {
            let _ = GetWindowPlacement::<HWND>(handle, &mut lpwndpl);
            // SHOW_WINDOW_CMD(2) == minimized
            return lpwndpl.showCmd != SHOW_WINDOW_CMD(2);
        }
    }

    false
}

pub fn show_search_bar(window: &Window) {
    let _ = window.show();
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
