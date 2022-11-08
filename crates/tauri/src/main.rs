#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
#[allow(unused_imports)]
use std::borrow::Cow;
use std::io;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use auto_launch::AutoLaunchBuilder;
use rpc::RpcMutex;
use tauri::{
    AppHandle, GlobalShortcutManager, Manager, PathResolver, RunEvent, SystemTray, SystemTrayEvent,
    Window,
};
use tokio::sync::broadcast;
use tokio::time::Duration;
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

#[cfg(target_os = "macos")]
use cocoa::appkit::NSWindow;

use shared::config::Config;
use spyglass_rpc::RpcClient;

mod cmd;
mod constants;
mod menu;
use menu::MenuID;
mod plugins;
mod rpc;
mod window;
use window::{
    show_connection_manager_window, show_crawl_stats_window, show_lens_manager_window,
    show_plugin_manager, show_search_bar, show_user_settings, show_wizard_window,
};

use crate::window::show_update_window;

#[derive(Clone)]
pub struct AppShutdown;
type PauseState = AtomicBool;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = tauri::generate_context!();
    let config = Config::new();
    #[cfg(not(debug_assertions))]
    let _guard = if config.user_settings.disable_telemetry {
        None
    } else {
        Some(sentry::init((
            "https://13d7d51a8293459abd0aba88f99f4c18@o1334159.ingest.sentry.io/6600471",
            sentry::ClientOptions {
                release: Some(Cow::from(ctx.package_info().version.to_string())),
                ..Default::default()
            },
        )))
    };

    // Check and register this app to run on boot
    if !config.user_settings.disable_autolaunch {
        if let Ok(path) = std::env::current_exe() {
            if let Some(path) = path.to_str() {
                if let Ok(auto) = AutoLaunchBuilder::new()
                    .set_app_name("com.athlabs.spyglass")
                    .set_app_path(path)
                    .set_use_launch_agent(true)
                    .build()
                {
                    if let Err(e) = auto.enable() {
                        log::warn!("Unable to add spyglass to startup items: {}", e);
                    }
                }
            }
        }
    }

    let file_appender = tracing_appender::rolling::daily(config.logs_dir(), "client.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with(
            fmt::Layer::new()
                .with_thread_names(true)
                .with_writer(io::stdout),
        )
        .with(fmt::Layer::new().with_ansi(false).with_writer(non_blocking));

    tracing::subscriber::set_global_default(subscriber).expect("Unable to set a global subscriber");
    LogTracer::init()?;

    let app = tauri::Builder::default()
        .plugin(plugins::lens_updater::init())
        .plugin(plugins::startup::init())
        .invoke_handler(tauri::generate_handler![
            cmd::authorize_connection,
            cmd::choose_folder,
            cmd::crawl_stats,
            cmd::delete_doc,
            cmd::delete_domain,
            cmd::escape,
            cmd::install_lens,
            cmd::list_connections,
            cmd::list_plugins,
            cmd::load_user_settings,
            cmd::network_change,
            cmd::open_folder_path,
            cmd::open_lens_folder,
            cmd::open_plugins_folder,
            cmd::open_result,
            cmd::open_settings_folder,
            cmd::recrawl_domain,
            cmd::resize_window,
            cmd::revoke_connection,
            cmd::resync_connection,
            cmd::save_user_settings,
            cmd::search_docs,
            cmd::search_lenses,
            cmd::toggle_plugin,
            cmd::update_and_restart,
        ])
        .menu(menu::get_app_menu(&ctx))
        .system_tray(SystemTray::new().with_menu(menu::get_tray_menu(&ctx, &config.clone())))
        .setup(move |app| {
            let app_handle = app.app_handle();
            window::show_startup_window(&app_handle);

            let (shutdown_tx, _) = broadcast::channel::<AppShutdown>(1);
            app.manage(shutdown_tx);

            let config = Config::new();
            log::info!("Loading prefs from: {:?}", Config::prefs_dir());

            // Copy default plugins to data directory to be picked up by the backend
            if let Err(e) = copy_plugins(&config, app.path_resolver()) {
                log::error!("Unable to copy default plugins: {}", e);
            }

            // macOS: hide from dock (also hides menu bar)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let window = app.get_window(constants::SEARCH_WIN_NAME).expect("Main window not found");
            window::center_search_bar(&window);
            // Hide on start.
            let _ = window.hide();

            // macOS: Handle multiple spaces correctly
            #[cfg(target_os = "macos")]
            {
                unsafe {
                    let ns_window = window.ns_window().expect("Unable to get ns_window") as cocoa::base::id;
                    ns_window.setCollectionBehavior_(cocoa::appkit::NSWindowCollectionBehavior::NSWindowCollectionBehaviorMoveToActiveSpace);
                }
            }

            // Spawn a version checking background task. Check every couple hours
            // for a new version.
            tauri::async_runtime::spawn(check_version_interval(window.clone()));

            // Load user settings
            app.manage(config.clone());
            app.manage(Arc::new(PauseState::new(false)));

            // Register global shortcut
            let window_clone = window.clone();
            let mut shortcuts = window.app_handle().global_shortcut_manager();
            match shortcuts.is_registered(&config.user_settings.shortcut) {
                Ok(is_registered) => {
                    if !is_registered
                    {
                        log::info!("Registering {} as shortcut", &config.user_settings.shortcut);
                        if let Err(e) = shortcuts
                            .register(&config.user_settings.shortcut, move || {
                                let window = window_clone.clone();
                                let _ = window.is_visible()
                                    .map(|is_visible| {
                                        if is_visible {
                                            window::hide_search_bar(&window);
                                        } else {
                                            window::show_search_bar(&window);
                                        }
                                    });
                            }) {
                            window::alert(&window, "Error registering global shortcut", &format!("{}", e));
                        }
                    }
                }
                Err(e) => window::alert(&window_clone, "Error registering global shortcut", &format!("{}", e))
            }

            Ok(())
        })
        .on_window_event(|event| {
            let window = event.window();
            if window.label() == "main" {
                if let tauri::WindowEvent::Focused(is_focused) = event.event() {
                    if !is_focused {
                        let handle = event.window();
                        window::hide_search_bar(handle);
                    }
                }
            }
        })
        .on_system_tray_event(move |app, event| {
            match event {
                // Only occurs on Windows.
                SystemTrayEvent::DoubleClick { .. } => {
                    let window = app.get_window("main")
                        .expect("Main window not initialized");
                    show_search_bar(&window);
                }
                SystemTrayEvent::MenuItemClick { id, .. } => {
                    let window = app.get_window("main").expect("Main window not initialized");

                    if let Ok(menu_id) = MenuID::from_str(&id) {
                        match menu_id {
                            MenuID::CRAWL_STATUS => {
                                // Don't block main thread when pausing the crawler.
                                let item_handle = app.tray_handle().get_item(&id);
                                let _ = item_handle.set_title("Handling request...");
                                let _ = item_handle.set_enabled(false);
                                tauri::async_runtime::spawn(pause_crawler(app.clone(), id.clone()));
                            }
                            MenuID::OPEN_CONNECTION_MANAGER => { show_connection_manager_window(app); },
                            MenuID::OPEN_LENS_MANAGER => { show_lens_manager_window(app); },
                            MenuID::OPEN_PLUGIN_MANAGER => { show_plugin_manager(app); },
                            MenuID::OPEN_LOGS_FOLDER => open_folder(config.logs_dir()),
                            MenuID::OPEN_SETTINGS_MANAGER => { show_user_settings(app) },
                            MenuID::OPEN_WIZARD => { show_wizard_window(app); }
                            MenuID::SHOW_CRAWL_STATUS => {
                                show_crawl_stats_window(app);
                            }
                            MenuID::SHOW_SEARCHBAR => {
                                let _ = window.is_visible()
                                    .map(|is_visible| {
                                        if !is_visible {
                                            window::hide_search_bar(&window);
                                        }
                                    });
                            }
                            MenuID::QUIT => app.exit(0),
                            MenuID::DEV_SHOW_CONSOLE => window.open_devtools(),
                            MenuID::JOIN_DISCORD => {
                                let _ = open::that(shared::constants::DISCORD_JOIN_URL);
                            }
                            // Just metainfo
                            MenuID::VERSION => {},
                        }
                    }
                },
                _ => ()
            }
        })
        .build(ctx)
        .expect("error while running tauri application");

    app.run(|app_handle, e| match e {
        RunEvent::ExitRequested { .. } => {
            // Do some cleanup for long running tasks
            let shutdown_tx = app_handle.state::<broadcast::Sender<AppShutdown>>();
            let _ = shutdown_tx.send(AppShutdown);
        }
        RunEvent::Exit { .. } => {
            log::info!("😔 bye bye");
        }
        _ => {}
    });

    Ok(())
}

async fn pause_crawler(app: AppHandle, menu_id: String) {
    if let Some(rpc) = app.try_state::<RpcMutex>() {
        let pause_state = app.state::<Arc<PauseState>>().inner();
        let rpc = rpc.lock().await;
        let is_paused = pause_state.clone();

        match rpc
            .client
            .toggle_pause(!is_paused.load(Ordering::Relaxed))
            .await
        {
            Ok(_) => {
                let is_paused = !pause_state.load(Ordering::Relaxed);
                pause_state.store(is_paused, Ordering::Relaxed);

                let new_label = if is_paused {
                    "▶️ Resume indexing"
                } else {
                    "⏸ Pause indexing"
                };

                let item_handle = app.tray_handle().get_item(&menu_id);
                let _ = item_handle.set_title(new_label);
                let _ = item_handle.set_enabled(true);
            }
            Err(err) => log::error!("Error sending RPC: {}", err),
        }
    }
}

fn open_folder(folder: PathBuf) {
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open")
        .arg(folder)
        .spawn()
        .expect("xdg-open cmd not available");

    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg(folder)
        .spawn()
        .expect("open cmd not available");

    #[cfg(target_os = "windows")]
    std::process::Command::new("explorer")
        .arg(folder)
        .spawn()
        .expect("explorer cmd not available");
}

async fn check_version_interval(window: Window) {
    let mut interval =
        tokio::time::interval(Duration::from_secs(constants::VERSION_CHECK_INTERVAL_S));

    let app_handle = window.app_handle();
    let shutdown_tx = app_handle.state::<broadcast::Sender<AppShutdown>>();
    let mut shutdown = shutdown_tx.subscribe();

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                log::info!("🛑 Shutting down version checker");
                return;
            },
            _ = interval.tick() => {
                log::info!("checking for update...");
                if let Ok(response) = app_handle.updater().check().await {
                    if response.is_update_available() {
                        // show update dialog
                        show_update_window(&app_handle);
                    }
                }
            }
        }
    }
}

fn copy_plugins(config: &Config, resolver: PathResolver) -> anyhow::Result<()> {
    // Copy default plugins to data directory to be picked up by the backend
    let plugin_path = resolver.resolve_resource("../../assets/plugins");
    let base_plugin_dir = config.plugins_dir();

    if let Some(plugin_path) = plugin_path {
        for entry in std::fs::read_dir(plugin_path)? {
            let path = entry?.path();
            if path.is_dir() {
                let plugin_name = path.file_name().expect("Unable to parse folder");
                let plugin_dir = base_plugin_dir.join(plugin_name);
                // Create folder for plugin
                std::fs::create_dir_all(plugin_dir.clone())?;
                // Copy plugin contents to folder
                for file in std::fs::read_dir(path)? {
                    let file = file?;
                    let new_file_path = plugin_dir.join(file.file_name());
                    std::fs::copy(file.path(), new_file_path)?;
                }
            }
        }
    }

    Ok(())
}
