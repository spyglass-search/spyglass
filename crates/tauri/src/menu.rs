use tauri::{CustomMenuItem, Menu, MenuItem, Submenu, SystemTrayMenu, SystemTrayMenuItem};

pub const TOGGLE_MENU_ITEM: &str = "toggle";
pub const QUIT_MENU_ITEM: &str = "quit";

pub const NUM_DOCS_MENU_ITEM: &str = "num_docs";
pub const NUM_QUEUED_MENU_ITEM: &str = "num_queue";
pub const CRAWL_STATUS_MENU_ITEM: &str = "crawl_status";

pub const OPEN_LENSES_FOLDER: &str = "open_lenses_folder";
pub const OPEN_SETTINGS_FOLDER: &str = "open_settings_folder";
pub const OPEN_LOGS_FOLDER: &str = "open_logs_folder";

pub fn get_tray_menu() -> SystemTrayMenu {
    let ctx = tauri::generate_context!();

    let pause = CustomMenuItem::new(CRAWL_STATUS_MENU_ITEM.to_string(), "");
    let quit = CustomMenuItem::new(QUIT_MENU_ITEM.to_string(), "Quit");

    let open_lenses_folder =
        CustomMenuItem::new(OPEN_LENSES_FOLDER.to_string(), "Show lenses folder");
    let open_settings_folder =
        CustomMenuItem::new(OPEN_SETTINGS_FOLDER.to_string(), "Show settings folder");

    let open_logs_folder = CustomMenuItem::new(OPEN_LOGS_FOLDER.to_string(), "Show logs folder");

    SystemTrayMenu::new()
        .add_item(pause)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(
            CustomMenuItem::new("about", format!("v20{}", ctx.package_info().version)).disabled(),
        )
        .add_item(
            CustomMenuItem::new(NUM_DOCS_MENU_ITEM.to_string(), "XX documents indexed").disabled(),
        )
        .add_item(CustomMenuItem::new(NUM_QUEUED_MENU_ITEM.to_string(), "XX queued").disabled())
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(open_lenses_folder)
        .add_item(open_settings_folder)
        .add_item(open_logs_folder)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit)
}

pub fn get_app_menu() -> Menu {
    let ctx = tauri::generate_context!();

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
