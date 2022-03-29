use tauri::{CustomMenuItem, Menu, MenuItem, Submenu, SystemTrayMenu, SystemTrayMenuItem};

pub const TOGGLE_MENU_ITEM: &str = "toggle";
pub const QUIT_MENU_ITEM: &str = "quit";

pub const NUM_DOCS_MENU_ITEM: &str = "num_docs";
pub const NUM_QUEUED_MENU_ITEM: &str = "num_queue";
pub const CRAWL_STATUS_MENU_ITEM: &str = "crawl_status";

pub fn get_tray_menu() -> SystemTrayMenu {
    let pause = CustomMenuItem::new(CRAWL_STATUS_MENU_ITEM.to_string(), "");
    let hide = CustomMenuItem::new(TOGGLE_MENU_ITEM.to_string(), "Hide");
    let quit = CustomMenuItem::new(QUIT_MENU_ITEM.to_string(), "Quit");

    SystemTrayMenu::new()
        .add_item(pause)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new(NUM_DOCS_MENU_ITEM.to_string(), "XX documents indexed").disabled())
        .add_item(CustomMenuItem::new(NUM_QUEUED_MENU_ITEM.to_string(), "XX queued").disabled())
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(hide)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit)
}

pub fn get_app_menu() -> Menu {
    let ctx = tauri::generate_context!();

    Menu::new().add_submenu(Submenu::new(
        &ctx.package_info().name,
        Menu::new()
            .add_native_item(MenuItem::About(ctx.package_info().name.to_string(), Default::default()))
            // Currently we need to include these so that the shortcuts for these
            // actions work.
            .add_native_item(MenuItem::Copy)
            .add_native_item(MenuItem::Paste)
            .add_native_item(MenuItem::SelectAll)
            .add_native_item(MenuItem::Separator)
            .add_native_item(MenuItem::Quit),
    ))
}
