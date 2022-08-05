use std::process::Command;
use shared::event::ClientEvent;
use tauri::api::dialog::{MessageDialogBuilder, MessageDialogButtons, MessageDialogKind};
use tauri::{AppHandle, LogicalSize, Manager, Size, Window, WindowBuilder, WindowUrl};

use crate::constants;

pub fn center_window(window: &Window) {
    if let Some(monitor) = window.primary_monitor().unwrap() {
        let size = monitor.size();
        let scale = monitor.scale_factor();

        let middle = (size.width as f64 / (scale * 2.0)) - (constants::INPUT_WIDTH / 2.0);

        window
            .set_position(tauri::Position::Logical(tauri::LogicalPosition {
                x: middle,
                y: constants::INPUT_Y,
            }))
            .unwrap();
    }
}

pub fn hide_window(window: &Window) {
    window.hide().unwrap();
    window
        .emit(ClientEvent::ClearSearch.as_ref(), true)
        .unwrap();
}

pub async fn resize_window(window: &Window, height: f64) {
    window
        .set_size(Size::Logical(LogicalSize {
            width: constants::INPUT_WIDTH,
            height,
        }))
        .unwrap();
}

pub fn show_window(window: &Window) {
    window
        .emit(ClientEvent::FocusWindow.as_ref(), true)
        .unwrap();
    window.show().unwrap();
    window.set_focus().unwrap();
    window.set_always_on_top(true).unwrap();
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
        .unwrap()
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
        .min_inner_size(450.0, 350.0)
        .build()
        .unwrap()
    };

    // A little hack to bring window to the front if its hiding behind something.
    let _ = window.set_always_on_top(true);
    let _ = window.set_always_on_top(false);
}

pub fn alert(window: &Window, title: &str, message: &str) {
    MessageDialogBuilder::new(title, message)
        .parent(window)
        .buttons(MessageDialogButtons::Ok)
        .kind(MessageDialogKind::Error)
        .show(|_| {});
}

pub fn notify(title: &str, body: &str) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let title = title.to_string().clone();
        let body = body.to_string().clone();
        tauri::async_runtime::spawn(async move {
            // osascript -e 'display notification "hello world!" with title "test"'
            Command::new("osascript")
                .arg("-e")
                .arg(format!("display notification \"{}\" with title \"{}\"", body, title))
                .spawn()
                .expect("Failed to send message");
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        use notify_rust::Notification;
        Notification::new()
            .summary(title)
            .body(body)
            .show()?;
    }

    Ok(())
}