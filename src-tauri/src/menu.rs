use tauri::{CustomMenuItem, Menu, MenuItem, Submenu, SystemTrayMenu, SystemTrayMenuItem};

pub fn get_tray_menu() -> SystemTrayMenu {
    let pause = CustomMenuItem::new("pause".to_string(), "Pause Crawler");
    let hide = CustomMenuItem::new("toggle".to_string(), "Hide");
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");

    SystemTrayMenu::new()
        .add_item(pause)
        .add_item(hide)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit)
}

pub fn get_app_menu() -> Menu {
    let ctx = tauri::generate_context!();

    Menu::new().add_submenu(Submenu::new(
        &ctx.package_info().name,
        Menu::new()
            .add_native_item(MenuItem::About(ctx.package_info().name.to_string()))
            // Currently we need to include these so that the shortcuts for these
            // actions work.
            .add_native_item(MenuItem::Copy)
            .add_native_item(MenuItem::Paste)
            .add_native_item(MenuItem::SelectAll)
            .add_native_item(MenuItem::Separator)
            .add_native_item(MenuItem::Quit),
    ))
}
