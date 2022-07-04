use shared::config::Config;
use tauri::{
    utils::assets::EmbeddedAssets, Context, CustomMenuItem, Menu, MenuItem, Submenu,
    SystemTrayMenu, SystemTrayMenuItem,
};

pub const VERSION_MENU_ITEM: &str = "version";
pub const QUIT_MENU_ITEM: &str = "quit";

pub const NUM_DOCS_MENU_ITEM: &str = "num_docs";
pub const CRAWL_STATUS_MENU_ITEM: &str = "crawl_status";

pub const OPEN_LENS_MANAGER: &str = "open_lens_manager";
pub const OPEN_SETTINGS_FOLDER: &str = "open_settings_folder";
pub const OPEN_LOGS_FOLDER: &str = "open_logs_folder";
pub const SHOW_SEARCHBAR: &str = "show_searchbar";
pub const SHOW_CRAWL_STATUS: &str = "show_crawl_status_window";
pub const JOIN_DISCORD: &str = "join_discord";

pub const DEV_SHOW_CONSOLE: &str = "dev_show_console";

pub fn get_tray_menu(ctx: &Context<EmbeddedAssets>, config: &Config) -> SystemTrayMenu {
    let show = CustomMenuItem::new(SHOW_SEARCHBAR.to_string(), "Show search")
        .accelerator(config.user_settings.shortcut.clone());

    let pause = CustomMenuItem::new(CRAWL_STATUS_MENU_ITEM.to_string(), "");
    let quit = CustomMenuItem::new(QUIT_MENU_ITEM.to_string(), "Quit");

    let open_settings_folder =
        CustomMenuItem::new(OPEN_SETTINGS_FOLDER.to_string(), "Open settings folder");

    let open_logs_folder = CustomMenuItem::new(OPEN_LOGS_FOLDER.to_string(), "Open logs folder");

    let app_version = format!("v20{}", ctx.package_info().version);
    let mut tray = SystemTrayMenu::new();

    tray = tray
        .add_item(show)
        .add_item(pause)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new(VERSION_MENU_ITEM, app_version).disabled())
        .add_item(
            CustomMenuItem::new(NUM_DOCS_MENU_ITEM.to_string(), "XX documents indexed").disabled(),
        )
        .add_item(CustomMenuItem::new(
            SHOW_CRAWL_STATUS.to_string(),
            "Show crawl status",
        ))
        .add_item(CustomMenuItem::new(
            OPEN_LENS_MANAGER.to_string(),
            "Manage/install lenses",
        ))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(open_settings_folder)
        .add_item(open_logs_folder);

    // Add dev utils
    if cfg!(debug_assertions) {
        tray = tray
            .add_native_item(SystemTrayMenuItem::Separator)
            .add_item(CustomMenuItem::new(DEV_SHOW_CONSOLE, "Open dev console"));
    }

    tray.add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new(JOIN_DISCORD, "Join our Discord"))
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
