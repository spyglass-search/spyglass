use tauri::{CustomMenuItem, Menu, MenuItem, Submenu, SystemTrayMenu, SystemTrayMenuItem};

pub fn get_tray_menu() -> SystemTrayMenu {
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let hide = CustomMenuItem::new("toggle".to_string(), "Hide");

    SystemTrayMenu::new()
        .add_item(hide)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit)
}

pub fn get_app_menu() -> Menu {
    let ctx = tauri::generate_context!();

    Menu::new().add_submenu(Submenu::new(
        &ctx.package_info().name,
        Menu::new()
            .add_native_item(MenuItem::Quit),
    ))
}
