use crate::{constants, platform};
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

pub fn center_search_bar(window: &Window) {
    if let Some(monitor) = find_monitor(window) {
        let size = monitor.size();
        let scale = monitor.scale_factor();

        let middle = (size.width as f64 / (scale * 2.0)) - (constants::INPUT_WIDTH / 2.0);

        let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition {
            x: middle,
            y: constants::INPUT_Y,
        }));
    } else {
        log::warn!("Unable to detect any monitors.");
    }
}

pub fn show_search_bar(window: &Window) {
    #[cfg(target_os = "linux")]
    platform::linux::show_search_bar(window);

    #[cfg(target_os = "macos")]
    platform::mac::show_search_bar(window);

    #[cfg(target_os = "windows")]
    platform::windows::show_search_bar(window);
}

pub fn hide_search_bar(window: &Window) {
    #[cfg(target_os = "linux")]
    platform::linux::hide_search_bar(window);

    #[cfg(target_os = "macos")]
    platform::mac::hide_search_bar(window);

    #[cfg(target_os = "windows")]
    platform::windows::hide_search_bar(window);
}

pub async fn resize_window(window: &Window, height: f64) {
    let monitor_height = {
        if let Some(monitor) = find_monitor(window) {
            let size = monitor.size();
            let scale = monitor.scale_factor();
            Some((size.height as f64) / scale - (constants::INPUT_Y * 2.0))
        } else {
            None
        }
    };

    // If the requested height is greater than the monitor size, use the monitor
    // height so we don't go offscreen.
    let height = if let Some(monitor_height) = monitor_height {
        monitor_height.min(height)
    } else {
        height
    };

    let _ = window.set_size(Size::Logical(LogicalSize {
        width: constants::INPUT_WIDTH,
        height,
    }));
}

fn show_window(window: &Window) {
    let _ = window.show();
    // A little hack to bring window to the front if its hiding behind something.
    let _ = window.set_always_on_top(true);
    let _ = window.set_always_on_top(false);
    let _ = window.set_focus();
    let _ = window.center();
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

pub fn show_connection_manager_window(app: &AppHandle) {
    _show_tab(app, "/settings/connections");
}

pub fn show_discover_window(app: &AppHandle) {
    _show_tab(app, "/settings/discover");
}

pub fn show_lens_manager_window(app: &AppHandle) {
    _show_tab(app, "/settings/library");
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

    show_window(&window);
}

pub fn show_startup_window(app: &AppHandle) -> Window {
    let window = if let Some(window) = app.get_window(constants::STARTUP_WIN_NAME) {
        window
    } else {
        WindowBuilder::new(
            app,
            constants::STARTUP_WIN_NAME,
            WindowUrl::App("/startup".into()),
        )
        .title("Spyglass - Starting up")
        .decorations(false)
        .min_inner_size(256.0, 272.0)
        .max_inner_size(256.0, 272.0)
        .transparent(true)
        .build()
        .expect("Unable to build startup window")
    };

    show_window(&window);
    window
}

pub fn show_wizard_window(app: &AppHandle) {
    let window = if let Some(window) = app.get_window(constants::WIZARD_WIN_NAME) {
        window
    } else {
        WindowBuilder::new(
            app,
            constants::WIZARD_WIN_NAME,
            WindowUrl::App("/wizard".into()),
        )
        .title("Spyglass - Wizard")
        .min_inner_size(480.0, 440.0)
        .max_inner_size(480.0, 440.0)
        .build()
        .expect("Unable to build window for wizard")
    };

    show_window(&window);
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
