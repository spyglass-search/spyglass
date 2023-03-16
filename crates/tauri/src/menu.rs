use shared::config::UserSettings;
use strum_macros::{Display, EnumString};
use tauri::{
    CustomMenuItem, Menu, PackageInfo, SystemTrayMenu, SystemTrayMenuItem, SystemTraySubmenu,
};
#[cfg(not(target_os = "linux"))]
use tauri::{MenuItem, Submenu};

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
    OPEN_PLUGIN_MANAGER,
    OPEN_SETTINGS_MANAGER,
    OPEN_WIZARD,
    QUIT,
    SHOW_SEARCHBAR,
    VERSION,

    INSTALL_CHROME_EXT,
    INSTALL_FIREFOX_EXT,
}

pub fn get_tray_menu(package_info: &PackageInfo, user_settings: &UserSettings) -> SystemTrayMenu {
    let show = CustomMenuItem::new(MenuID::SHOW_SEARCHBAR.to_string(), "Show search")
        .accelerator(user_settings.shortcut.clone());

    let pause = CustomMenuItem::new(MenuID::CRAWL_STATUS.to_string(), "⏸ Pause indexing");
    let quit = CustomMenuItem::new(MenuID::QUIT.to_string(), "Quit");

    let open_logs_folder =
        CustomMenuItem::new(MenuID::OPEN_LOGS_FOLDER.to_string(), "Open logs folder");

    let app_version: String = if cfg!(debug_assertions) {
        "🚧 dev-build 🚧".into()
    } else {
        format!("v20{}", package_info.version)
    };

    let mut tray = SystemTrayMenu::new();

    let settings_menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new(
            MenuID::OPEN_CONNECTION_MANAGER.to_string(),
            "Connections",
        ))
        .add_item(CustomMenuItem::new(
            MenuID::OPEN_PLUGIN_MANAGER.to_string(),
            "Plugins",
        ))
        .add_item(CustomMenuItem::new(
            MenuID::OPEN_SETTINGS_MANAGER.to_string(),
            "User settings",
        ));

    tray = tray
        .add_item(show)
        .add_item(pause)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new(MenuID::VERSION.to_string(), app_version).disabled())
        .add_item(CustomMenuItem::new(
            MenuID::DISCOVER.to_string(),
            "Discover Lenses",
        ))
        .add_item(CustomMenuItem::new(
            MenuID::OPEN_LENS_MANAGER.to_string(),
            "My Library",
        ))
        .add_submenu(SystemTraySubmenu::new("Settings", settings_menu))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(open_logs_folder);

    // Add dev utils
    if cfg!(debug_assertions) {
        tray = tray
            .add_native_item(SystemTrayMenuItem::Separator)
            .add_item(CustomMenuItem::new(
                MenuID::DEV_SHOW_CONSOLE.to_string(),
                "Open dev console",
            ));
    }

    tray.add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new(
            MenuID::OPEN_WIZARD.to_string(),
            "Getting Started Wizard",
        ))
        .add_item(CustomMenuItem::new(
            MenuID::JOIN_DISCORD.to_string(),
            "Join our Discord",
        ))
        .add_item(CustomMenuItem::new(
            MenuID::INSTALL_CHROME_EXT.to_string(),
            "Install Chrome Extension",
        ))
        .add_item(CustomMenuItem::new(
            MenuID::INSTALL_FIREFOX_EXT.to_string(),
            "Install Firefox Extension",
        ))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit)
}

pub fn get_app_menu() -> Menu {
    #[cfg(target_os = "linux")]
    return Menu::new();

    #[cfg(not(target_os = "linux"))]
    Menu::new().add_submenu(Submenu::new(
        "Spyglass".to_string(),
        Menu::new()
            .add_native_item(MenuItem::About("Spyglass".to_string(), Default::default()))
            // Currently we need to include these so that the shortcuts for these
            // actions work.
            .add_native_item(MenuItem::Copy)
            .add_native_item(MenuItem::Paste)
            .add_native_item(MenuItem::SelectAll)
            .add_native_item(MenuItem::Separator)
            .add_native_item(MenuItem::Hide)
            .add_native_item(MenuItem::Quit),
    ))
}
