use crate::constants;
use shared::event::ClientEvent;
use tauri::api::dialog::{MessageDialogBuilder, MessageDialogButtons, MessageDialogKind};
use tauri::{AppHandle, LogicalSize, Manager, Monitor, Size, Window, WindowBuilder, WindowUrl};

/// Try and detect which monitor the window is on so that we can determine the
/// screen size
fn find_monitor(window: &Window) -> Option<Monitor> {
    if let Ok(Some(mon)) = window.primary_monitor() {
        Some(mon)
    } else if let Ok(Some(mon)) = window.current_monitor() {
        Some(mon)
    } else if let Ok(mut monitors) = window.available_monitors() {
        if monitors.is_empty() {
            None
        } else {
            monitors.pop()
        }
    } else {
        None
    }
}

pub fn center_window(window: &Window) {
    if let Some(monitor) = find_monitor(window) {
        let size = monitor.size();
        let scale = monitor.scale_factor();

        let middle = (size.width as f64 / (scale * 2.0)) - (constants::INPUT_WIDTH / 2.0);

        let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition {
            x: middle,
            y: constants::INPUT_Y,
        }));
    } else {
        log::error!("Unable to detect any monitors.");
    }
}

pub fn hide_window(window: &Window) {
    let _ = window.hide();
    let _ = window.emit(ClientEvent::ClearSearch.as_ref(), true);
}

pub async fn resize_window(window: &Window, height: f64) {
    let window_height = {
        if let Some(monitor) = find_monitor(window) {
            let size = monitor.size();
            let scale = monitor.scale_factor();
            Some((size.height as f64) / scale - constants::INPUT_Y)
        } else {
            None
        }
    };

    let max_height = if let Some(window_height) = window_height {
        window_height.min(height)
    } else {
        height
    };

    let _ = window.set_size(Size::Logical(LogicalSize {
        width: constants::INPUT_WIDTH,
        height: max_height
    }));
}

pub fn show_window(window: &Window) {
    let _ = window.emit(ClientEvent::FocusWindow.as_ref(), true);
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.set_always_on_top(true);
    center_window(window);
}

fn _show_tab(app: &AppHandle, tab_url: &str) {
    let window = if let Some(window) = app.get_window(constants::SETTINGS_WIN_NAME) {
        window
    } else {
        WindowBuilder::new(
            app,
            constants::SETTINGS_WIN_NAME,
            WindowUrl::App(tab_url.into()),
        )
        .title("Spyglass - Personal Search Engine")
        .min_inner_size(constants::MIN_WINDOW_WIDTH, constants::MIN_WINDOW_HEIGHT)
        .build()
        .expect("Unable to build window for settings")
    };

    let _ = window.emit(ClientEvent::Navigate.as_ref(), tab_url);
    // A little hack to bring window to the front if its hiding behind something.
    let _ = window.set_always_on_top(true);
    let _ = window.set_always_on_top(false);
}

pub fn show_crawl_stats_window(app: &AppHandle) {
    _show_tab(app, "/settings/stats");
}

pub fn show_lens_manager_window(app: &AppHandle) {
    _show_tab(app, "/settings/lenses");
}

pub fn show_plugin_manager(app: &AppHandle) {
    _show_tab(app, "/settings/plugins");
}

pub fn show_user_settings(app: &AppHandle) {
    _show_tab(app, "/settings/user");
}

pub fn show_update_window(app: &AppHandle) {
    let window = if let Some(window) = app.get_window(constants::UPDATE_WIN_NAME) {
        window
    } else {
        WindowBuilder::new(
            app,
            constants::UPDATE_WIN_NAME,
            WindowUrl::App("/updater".into()),
        )
        .title("Spyglass - Update Available!")
        .min_inner_size(450.0, 375.0)
        .max_inner_size(450.0, 375.0)
        .build()
        .expect("Unable to build window for updater")
    };

    // A little hack to bring window to the front if its hiding behind something.
    let _ = window.set_always_on_top(true);
    let _ = window.set_always_on_top(false);
    let _ = window.center();
}

pub fn alert(window: &Window, title: &str, message: &str) {
    MessageDialogBuilder::new(title, message)
        .parent(window)
        .buttons(MessageDialogButtons::Ok)
        .kind(MessageDialogKind::Error)
        .show(|_| {});
}

#[allow(dead_code)]
pub fn notify(_app: &AppHandle, title: &str, body: &str) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let title = title.to_string();
        let body = body.to_string();
        tauri::async_runtime::spawn(async move {
            // osascript -e 'display notification "hello world!" with title "test"'
            Command::new("osascript")
                .arg("-e")
                .arg(format!(
                    "display notification \"{}\" with title \"{}\"",
                    body, title
                ))
                .spawn()
                .expect("Failed to send message");
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        use tauri::api::notification::Notification;
        let _ = Notification::new(&_app.config().tauri.bundle.identifier)
            .title(title)
            .body(body)
            .show();
    }

    Ok(())
}
