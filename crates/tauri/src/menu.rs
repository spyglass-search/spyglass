use shared::config::Config;
use strum_macros::{Display, EnumString};
use tauri::{
    utils::assets::EmbeddedAssets, Context, CustomMenuItem, Menu, MenuItem, Submenu,
    SystemTrayMenu, SystemTrayMenuItem,
};

#[derive(Display, Debug, EnumString)]
#[allow(non_camel_case_types)]
pub enum MenuID {
    CRAWL_STATUS,
    DEV_SHOW_CONSOLE,
    JOIN_DISCORD,
    NUM_DOCS,
    OPEN_LENS_MANAGER,
    OPEN_LOGS_FOLDER,
    OPEN_PLUGIN_MANAGER,
    OPEN_SETTINGS_FOLDER,
    QUIT,
    SHOW_CRAWL_STATUS,
    SHOW_SEARCHBAR,
    VERSION,
}

pub fn get_tray_menu(ctx: &Context<EmbeddedAssets>, config: &Config) -> SystemTrayMenu {
    let show = CustomMenuItem::new(MenuID::SHOW_SEARCHBAR.to_string(), "Show search")
        .accelerator(config.user_settings.shortcut.clone());

    let pause = CustomMenuItem::new(MenuID::CRAWL_STATUS.to_string(), "");
    let quit = CustomMenuItem::new(MenuID::QUIT.to_string(), "Quit");

    let open_settings_folder = CustomMenuItem::new(
        MenuID::OPEN_SETTINGS_FOLDER.to_string(),
        "Open settings folder",
    );

    let open_logs_folder =
        CustomMenuItem::new(MenuID::OPEN_LOGS_FOLDER.to_string(), "Open logs folder");

    let app_version = format!("v20{}", ctx.package_info().version);
    let mut tray = SystemTrayMenu::new();

    tray = tray
        .add_item(show)
        .add_item(pause)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new(MenuID::VERSION.to_string(), app_version).disabled())
        .add_item(
            CustomMenuItem::new(MenuID::NUM_DOCS.to_string(), "XX documents indexed").disabled(),
        )
        .add_item(CustomMenuItem::new(
            MenuID::SHOW_CRAWL_STATUS.to_string(),
            "Show crawl status",
        ))
        .add_item(CustomMenuItem::new(
            MenuID::OPEN_LENS_MANAGER.to_string(),
            "Manage/install lenses",
        ))
        .add_item(CustomMenuItem::new(
            MenuID::OPEN_PLUGIN_MANAGER.to_string(),
            "Manage plugins",
        ))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(open_settings_folder)
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
            MenuID::JOIN_DISCORD.to_string(),
            "Join our Discord",
        ))
        .add_item(quit)
}

pub fn get_app_menu(ctx: &Context<EmbeddedAssets>) -> Menu {
    if cfg!(target_os = "linux") {
        return Menu::new();
    }

    Menu::new().add_submenu(Submenu::new(
        &ctx.package_info().name,
        Menu::new()
            .add_native_item(MenuItem::About(
                ctx.package_info().name.to_string(),
                Default::default(),
            ))
            // Currently we need to include these so that the shortcuts for these
            // actions work.
            .add_native_item(MenuItem::Copy)
            .add_native_item(MenuItem::Paste)
            .add_native_item(MenuItem::SelectAll)
            .add_native_item(MenuItem::Separator)
            .add_native_item(MenuItem::Quit),
    ))
}
