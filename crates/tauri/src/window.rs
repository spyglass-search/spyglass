use crate::constants::{TabLocation, SETTINGS_WIN_NAME};
use crate::menu::get_app_menu;
use crate::{constants, platform};
use shared::event::{ClientEvent, ModelStatusPayload};
use shared::metrics::Metrics;
use tauri::api::dialog::{MessageDialogBuilder, MessageDialogButtons, MessageDialogKind};
use tauri::{
    AppHandle, Manager, Menu, Monitor, PhysicalPosition, PhysicalSize, Size, Window, WindowBuilder,
    WindowEvent, WindowUrl,
};

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
    let window_size = match window.inner_size() {
        Ok(size) => size,
        // Nothing to do if the window is not created yet.
        Err(_) => return,
    };

    if let Some(monitor) = find_monitor(window) {
        let screen_position = monitor.position();
        let screen_size = monitor.size();

        let y = (constants::INPUT_Y * monitor.scale_factor()) as i32;
        let new_position = PhysicalPosition {
            x: screen_position.x
                + ((screen_size.width as i32 / 2) - (window_size.width as i32 / 2)),
            y,
        };

        let _ = window.set_position(tauri::Position::Physical(new_position));
    } else {
        log::warn!("Unable to detect any monitors.");
    }
}

pub fn show_search_bar(window: &Window) {
    platform::show_search_bar(window);

    // Wait a little bit for the window to show being focusing on it.
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(256)).await;
        let _ = window.emit(ClientEvent::FocusWindow.as_ref(), true);
    });
}

pub fn hide_search_bar(window: &Window) {
    let handle = window.app_handle();
    // don't hide if the settings window is open
    if let Some(settings_window) = handle.get_window(SETTINGS_WIN_NAME) {
        if settings_window.is_visible().unwrap_or_default() {
            return;
        }
    }

    platform::hide_search_bar(window);
}

/// Builds or returns the main searchbar window
pub fn get_searchbar(app: &AppHandle) -> Window {
    if let Some(window) = app.get_window(constants::SEARCH_WIN_NAME) {
        window
    } else {
        let window =
            WindowBuilder::new(app, constants::SEARCH_WIN_NAME, WindowUrl::App("/".into()))
                .menu(get_app_menu())
                .title("Spyglass")
                .decorations(false)
                .transparent(true)
                .visible(false)
                .disable_file_drop_handler()
                .inner_size(640.0, 108.0)
                .build()
                .expect("Unable to create searchbar window");

        // macOS: Handle multiple spaces correctly
        #[cfg(target_os = "macos")]
        {
            use cocoa::appkit::NSWindow;
            unsafe {
                let ns_window =
                    window.ns_window().expect("Unable to get ns_window") as cocoa::base::id;
                ns_window.setCollectionBehavior_(cocoa::appkit::NSWindowCollectionBehavior::NSWindowCollectionBehaviorMoveToActiveSpace);
            }
        }

        window
    }
}

pub async fn resize_window(window: &Window, height: f64) {
    let new_size = if let Some(monitor) = find_monitor(window) {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let monitor_height = size.height as f64 - (constants::INPUT_Y * scale);

        // If the requested height is greater than the monitor size, use the monitor
        // height so we don't go offscreen.
        let height = monitor_height.min(height);
        Size::Physical(PhysicalSize {
            width: (constants::INPUT_WIDTH * scale) as u32,
            height: (height * scale) as u32,
        })
    } else {
        log::warn!("Unable to detect monitor size, resizing using defaults");
        Size::Physical(PhysicalSize {
            width: constants::INPUT_WIDTH as u32,
            height: height as u32,
        })
    };

    // recenter after resize
    let _ = window.set_size(new_size);
    center_search_bar(window);
}

fn show_window(window: &Window) {
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.center();
}

pub fn navigate_to_tab(app: &AppHandle, tab_url: &TabLocation) {
    let tab_url = tab_url.to_string();

    let window = if let Some(window) = app.get_window(constants::SETTINGS_WIN_NAME) {
        window
    } else {
        WindowBuilder::new(
            app,
            constants::SETTINGS_WIN_NAME,
            WindowUrl::App(tab_url.clone().into()),
        )
        .title("Spyglass - Personal Search Engine")
        // Create an empty menu so now menubar shows up on Windows
        .menu(Menu::new())
        .min_inner_size(constants::MIN_WINDOW_WIDTH, constants::MIN_WINDOW_HEIGHT)
        .build()
        .expect("Unable to build window for settings")
    };

    let _ = window.emit(ClientEvent::Navigate.as_ref(), tab_url);
    show_window(&window);
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

pub fn update_progress_window(app: &AppHandle, msg: &str, progress: u8) -> Window {
    let window = if let Some(window) = app.get_window(constants::PROGRESS_WIN_NAME) {
        window
    } else {
        WindowBuilder::new(
            app,
            constants::PROGRESS_WIN_NAME,
            WindowUrl::App("/progress".into()),
        )
        .title("Download Progress")
        .menu(Menu::new())
        .resizable(true)
        .inner_size(300.0, 64.0)
        .build()
        .expect("Unable to build window for progress")
    };

    let payload = ModelStatusPayload {
        msg: msg.to_string(),
        percent: progress.to_string(),
    };

    log::debug!("emitting update: {:?}", payload);
    let _ = window.emit("progress_update", payload);
    let _ = window.show();
    let _ = window.set_focus();
    window
}

pub fn show_wizard_window(app: &AppHandle) {
    let window = if let Some(window) = app.get_window(constants::WIZARD_WIN_NAME) {
        window
    } else {
        let wizard_window = WindowBuilder::new(
            app,
            constants::WIZARD_WIN_NAME,
            WindowUrl::App("/wizard".into()),
        )
        .title("Getting Started")
        .menu(Menu::new())
        .min_inner_size(400.0, 492.0)
        .max_inner_size(400.0, 492.0)
        .build()
        .expect("Unable to build window for wizard");

        let window_copy = wizard_window.clone();
        wizard_window.on_window_event(move |evt| {
            if let WindowEvent::CloseRequested { api: _, .. } = evt {
                tauri::async_runtime::spawn(wizard_closed_event(window_copy.clone()));
            }
        });
        wizard_window
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
                    "display notification \"{body}\" with title \"{title}\""
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

// Helper method used to send the wizard closed event metric
async fn wizard_closed_event(window: tauri::Window) {
    if let Some(metrics) = window.app_handle().try_state::<Metrics>() {
        let current_version = crate::current_version(window.app_handle().package_info());
        metrics
            .track(shared::metrics::Event::WizardClosed { current_version })
            .await;
    }
}
