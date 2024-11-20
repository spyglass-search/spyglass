use std::str::FromStr;

use shared::config::{Config, UserSettings};
use strum_macros::{Display, EnumString};
use tauri::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    tray::{TrayIcon, TrayIconEvent},
    AppHandle, Manager, PackageInfo, Wry,
};

use crate::{pause_crawler, platform::os_open, window};

#[derive(Clone)]
pub struct MenuState {
    pub pause_toggle: MenuItem<Wry>,
}

#[derive(Display, Debug, EnumString)]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
pub enum MenuID {
    CRAWL_STATUS,
    DEV_SHOW_CONSOLE,
    DISCOVER,
    JOIN_DISCORD,
    OPEN_CONNECTION_MANAGER,
    OPEN_LENS_MANAGER,
    OPEN_LOGS_FOLDER,
    OPEN_SETTINGS_MANAGER,
    OPEN_WIZARD,
    QUIT,
    SHOW_SEARCHBAR,
    VERSION,

    INSTALL_CHROME_EXT,
    INSTALL_FIREFOX_EXT,
}

pub fn get_tray_menu(
    app: &AppHandle,
    package_info: &PackageInfo,
    user_settings: &UserSettings,
) -> Result<Menu<tauri::Wry>, tauri::Error> {
    let app_version: String = if cfg!(debug_assertions) {
        "🚧 dev-build 🚧".into()
    } else {
        format!("v20{}", package_info.version)
    };

    let tray = Menu::with_id(app, "tray-menu")?;
    let settings_menu = Submenu::with_items(
        app,
        "Settings",
        true,
        &[
            &MenuItem::with_id(
                app,
                MenuID::OPEN_CONNECTION_MANAGER.to_string(),
                "Connections",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app,
                MenuID::OPEN_SETTINGS_MANAGER.to_string(),
                "User settings",
                true,
                None::<&str>,
            )?,
        ],
    )?;

    let pause_status = MenuItem::with_id(
        app,
        MenuID::CRAWL_STATUS.to_string(),
        "⏸ Pause indexing",
        true,
        None::<&str>,
    )?;
    // manage the pause status menu item so we can update it later.
    app.manage(MenuState {
        pause_toggle: pause_status.clone(),
    });

    tray.append_items(&[
        &MenuItem::with_id(
            app,
            MenuID::SHOW_SEARCHBAR.to_string(),
            "Show search",
            true,
            Some(user_settings.shortcut.clone()),
        )?,
        &pause_status,
        &PredefinedMenuItem::separator(app)?,
        &MenuItem::with_id(
            app,
            MenuID::VERSION.to_string(),
            app_version,
            false,
            None::<&str>,
        )?,
        &MenuItem::with_id(
            app,
            MenuID::DISCOVER.to_string(),
            "Discover Lenses",
            true,
            None::<&str>,
        )?,
        &MenuItem::with_id(
            app,
            MenuID::OPEN_LENS_MANAGER.to_string(),
            "My Library",
            true,
            None::<&str>,
        )?,
        &settings_menu,
        &MenuItem::with_id(
            app,
            MenuID::OPEN_LOGS_FOLDER.to_string(),
            "Open logs folder",
            true,
            None::<&str>,
        )?,
    ])?;

    // Add dev utils
    if cfg!(debug_assertions) {
        tray.append(&PredefinedMenuItem::separator(app)?)?;
        tray.append(&MenuItem::with_id(
            app,
            MenuID::DEV_SHOW_CONSOLE.to_string(),
            "Open dev console",
            true,
            None::<&str>,
        )?)?;
    }

    tray.append(&PredefinedMenuItem::separator(app)?)?;
    tray.append_items(&[
        &MenuItem::with_id(
            app,
            MenuID::OPEN_WIZARD.to_string(),
            "Getting Started Wizard",
            true,
            None::<&str>,
        )?,
        &MenuItem::with_id(
            app,
            MenuID::JOIN_DISCORD.to_string(),
            "Join our Discord",
            true,
            None::<&str>,
        )?,
        &MenuItem::with_id(
            app,
            MenuID::INSTALL_CHROME_EXT.to_string(),
            "Install Chrome Extension",
            true,
            None::<&str>,
        )?,
        &MenuItem::with_id(
            app,
            MenuID::INSTALL_FIREFOX_EXT.to_string(),
            "Install Firefox Extension",
            true,
            None::<&str>,
        )?,
    ])?;
    tray.append(&PredefinedMenuItem::separator(app)?)?;
    tray.append(&MenuItem::with_id(
        app,
        MenuID::QUIT.to_string(),
        "Quit",
        true,
        None::<&str>,
    )?)?;

    Ok(tray)
}

pub fn get_app_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, tauri::Error> {
    if cfg!(target_os = "linux") {
        Menu::new(app)
    } else {
        let app_menu = Menu::new(app)?;
        let menu = Submenu::with_items(
            app,
            "Spyglass",
            true,
            &[
                &PredefinedMenuItem::about(app, Some("Spyglass"), Default::default())?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::hide(app, None)?,
                &PredefinedMenuItem::quit(app, None)?,
            ],
        )?;
        app_menu.append(&menu)?;
        app_menu.append(&Submenu::with_items(
            app,
            "Edit",
            true,
            &[
                // Currently we need to include these so that the shortcuts for these
                // actions work.
                &PredefinedMenuItem::copy(app, None)?,
                &PredefinedMenuItem::paste(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::select_all(app, None)?,
            ],
        )?)?;

        Ok(app_menu)
    }
}

pub fn handle_tray_icon_events(tray: &TrayIcon, event: TrayIconEvent) {
    // Only occurs on Windows.
    if let TrayIconEvent::DoubleClick { .. } = event {
        let window = window::get_searchbar(tray.app_handle());
        window::show_search_bar(&window);
    }
}

pub fn handle_tray_menu_events(app: &AppHandle, event: MenuEvent) {
    let menu_id = if let Ok(menu_id) = MenuID::from_str(event.id.as_ref()) {
        menu_id
    } else {
        return;
    };

    match menu_id {
        MenuID::CRAWL_STATUS => {
            // Don't block main thread when pausing the crawler.
            tauri::async_runtime::spawn(pause_crawler(app.clone()));
            if let Some(state) = app.try_state::<MenuState>() {
                let _ = state.pause_toggle.set_text("Handling request...");
                let _ = state.pause_toggle.set_enabled(false);
            }
        }
        MenuID::DISCOVER => {
            window::navigate_to_tab(app, &crate::constants::WindowLocation::Discover);
        }
        MenuID::OPEN_CONNECTION_MANAGER => {
            window::navigate_to_tab(app, &crate::constants::WindowLocation::Connections);
        }
        MenuID::OPEN_LENS_MANAGER => {
            window::navigate_to_tab(app, &crate::constants::WindowLocation::Library);
        }
        MenuID::OPEN_LOGS_FOLDER => {
            if let Some(config) = app.try_state::<Config>() {
                crate::open_folder(config.logs_dir())
            }
        }
        MenuID::OPEN_SETTINGS_MANAGER => {
            window::navigate_to_tab(app, &crate::constants::WindowLocation::UserSettings);
        }
        MenuID::OPEN_WIZARD => {
            window::show_wizard_window(app);
        }
        MenuID::SHOW_SEARCHBAR => {
            let window = window::get_searchbar(app);
            window::show_search_bar(&window);
        }
        MenuID::QUIT => app.exit(0),
        MenuID::DEV_SHOW_CONSOLE => {
            let window = window::get_searchbar(app);
            window.open_devtools();
        }
        MenuID::JOIN_DISCORD => {
            let _ = os_open(
                &url::Url::parse(shared::constants::DISCORD_JOIN_URL).expect("Invalid Discord URL"),
                None,
            );
        }
        MenuID::INSTALL_CHROME_EXT => {
            let _ = os_open(
                &url::Url::parse(shared::constants::CHROME_EXT_LINK)
                    .expect("Invalid Chrome extension URL"),
                None,
            );
        }
        MenuID::INSTALL_FIREFOX_EXT => {
            let _ = os_open(
                &url::Url::parse(shared::constants::FIREFOX_EXT_LINK)
                    .expect("Invalid Firefox extension URL"),
                None,
            );
        }
        // Just metainfo
        _ => {}
    }
}
