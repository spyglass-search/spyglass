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
use tauri::api::process::current_binary;
use tauri::{
    AppHandle, Env, GlobalShortcutManager, Manager, PathResolver, RunEvent, SystemTray,
    SystemTrayEvent, Window,
};
use tokio::sync::broadcast;
use tokio::time::Duration;
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

use diff::Diff;
use shared::config::{Config, UserSettings};
use shared::metrics::{Event, Metrics};
use spyglass_rpc::RpcClient;

#[cfg(target_os = "linux")]
use platform::linux::os_open;
#[cfg(target_os = "macos")]
use platform::mac::os_open;
#[cfg(target_os = "windows")]
use platform::windows::os_open;

mod cmd;
mod constants;
mod menu;
use menu::MenuID;
mod platform;
mod plugins;
mod rpc;
mod window;
use window::{
    show_connection_manager_window, show_lens_manager_window, show_plugin_manager, show_search_bar,
    show_update_window, show_user_settings, show_wizard_window,
};

use crate::window::get_searchbar;

const LOG_LEVEL: tracing::Level = tracing::Level::INFO;
#[cfg(not(debug_assertions))]
const SPYGLASS_LEVEL: &str = "spyglass_app=INFO";
#[cfg(debug_assertions)]
const SPYGLASS_LEVEL: &str = "spyglass_app=DEBUG";

#[derive(Clone)]
pub enum AppEvent {
    BackendConnected,
    Shutdown,
}
type PauseState = AtomicBool;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = tauri::generate_context!();
    let current_version = format!("v20{}", &ctx.package_info().version);
    let config = Config::new();

    #[cfg(not(debug_assertions))]
    let _guard = if config.user_settings.disable_telemetry {
        None
    } else {
        Some(sentry::init((
            "https://13d7d51a8293459abd0aba88f99f4c18@o1334159.ingest.sentry.io/6600471",
            sentry::ClientOptions {
                release: Some(Cow::from(ctx.package_info().version.to_string())),
                traces_sample_rate: 0.1,
                ..Default::default()
            },
        )))
    };

    update_auto_launch(&config.user_settings);

    let file_appender = tracing_appender::rolling::daily(config.logs_dir(), "client.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::registry()
        .with(
            EnvFilter::from_default_env()
                .add_directive(LOG_LEVEL.into())
                .add_directive(SPYGLASS_LEVEL.parse().expect("Invalid EnvFilter")),
        )
        .with(
            fmt::Layer::new()
                .with_thread_names(true)
                .with_writer(io::stdout),
        )
        .with(fmt::Layer::new().with_ansi(false).with_writer(non_blocking))
        .with(sentry_tracing::layer());

    tracing::subscriber::set_global_default(subscriber).expect("Unable to set a global subscriber");
    LogTracer::init()?;

    // Fixes path issues on macOS & Linux
    let _ = fix_path_env::fix();
    let app = tauri::Builder::default()
        .plugin(plugins::lens_updater::init())
        .plugin(plugins::notify::init())
        .plugin(plugins::startup::init())
        .invoke_handler(tauri::generate_handler![
            cmd::authorize_connection,
            cmd::choose_folder,
            cmd::copy_to_clipboard,
            cmd::default_indices,
            cmd::delete_doc,
            cmd::escape,
            cmd::get_library_stats,
            cmd::get_shortcut,
            cmd::list_connections,
            cmd::list_plugins,
            cmd::load_action_settings,
            cmd::load_user_settings,
            cmd::navigate,
            cmd::network_change,
            cmd::open_folder_path,
            cmd::open_lens_folder,
            cmd::open_plugins_folder,
            cmd::open_result,
            cmd::open_settings_folder,
            cmd::recrawl_domain,
            cmd::resize_window,
            cmd::resync_connection,
            cmd::revoke_connection,
            cmd::save_user_settings,
            cmd::search_docs,
            cmd::search_lenses,
            cmd::toggle_plugin,
            cmd::update_and_restart,
            cmd::wizard_finished,
        ])
        .menu(menu::get_app_menu())
        .system_tray(SystemTray::new().with_menu(menu::get_tray_menu(
            ctx.package_info(),
            &config.user_settings.clone(),
        )))
        .setup(move |app| {
            let app_handle = app.app_handle();
            let startup_win = window::show_startup_window(&app_handle);

            let (appevent_channel, _) = broadcast::channel::<AppEvent>(1);
            app.manage(appevent_channel);

            let config = Config::new();
            log::info!("Loading prefs from: {:?}", Config::prefs_dir());

            // Copy default plugins to data directory to be picked up by the backend
            if let Err(e) = copy_plugins(&config, app.path_resolver()) {
                log::error!("Unable to copy default plugins: {}", e);
            }

            // Spawn a version checking background task. Check every couple hours
            // for a new version.
            tauri::async_runtime::spawn(check_version_interval(
                current_version,
                app_handle.clone(),
            ));

            let _ = get_searchbar(&app_handle);

            app.manage(config.clone());
            app.manage(Arc::new(PauseState::new(false)));
            app.manage(shared::metrics::Metrics::new(
                &Config::machine_identifier(),
                config.user_settings.disable_telemetry,
            ));

            register_global_shortcut(&startup_win, &app_handle, &config.user_settings);

            Ok(())
        })
        .on_system_tray_event(move |app, event| {
            match event {
                // Only occurs on Windows.
                SystemTrayEvent::DoubleClick { .. } => {
                    let window = window::get_searchbar(app);
                    show_search_bar(&window);
                }
                SystemTrayEvent::MenuItemClick { id, .. } => {
                    if let Ok(menu_id) = MenuID::from_str(&id) {
                        match menu_id {
                            MenuID::CRAWL_STATUS => {
                                // Don't block main thread when pausing the crawler.
                                let item_handle = app.tray_handle().get_item(&id);
                                let _ = item_handle.set_title("Handling request...");
                                let _ = item_handle.set_enabled(false);
                                tauri::async_runtime::spawn(pause_crawler(app.clone(), id.clone()));
                            }
                            MenuID::DISCOVER => {
                                window::show_discover_window(app);
                            }
                            MenuID::OPEN_CONNECTION_MANAGER => {
                                show_connection_manager_window(app);
                            }
                            MenuID::OPEN_LENS_MANAGER => {
                                show_lens_manager_window(app);
                            }
                            MenuID::OPEN_PLUGIN_MANAGER => {
                                show_plugin_manager(app);
                            }
                            MenuID::OPEN_LOGS_FOLDER => open_folder(config.logs_dir()),
                            MenuID::OPEN_SETTINGS_MANAGER => show_user_settings(app),
                            MenuID::OPEN_WIZARD => {
                                show_wizard_window(app);
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
                                    &url::Url::parse(shared::constants::DISCORD_JOIN_URL)
                                        .expect("Invalid Discord URL"),
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
                            MenuID::VERSION => {}
                        }
                    }
                }
                _ => (),
            }
        })
        .build(ctx)
        .expect("error while running tauri application");

    app.run(|app_handle, e| match e {
        RunEvent::ExitRequested { .. } => {
            // Do some cleanup for long running tasks
            let shutdown_tx = app_handle.state::<broadcast::Sender<AppEvent>>();
            let _ = shutdown_tx.send(AppEvent::Shutdown);
        }
        RunEvent::Exit { .. } => {
            log::info!("😔 bye bye");
        }
        _ => {}
    });

    Ok(())
}

// Applies updated configuration to the client
pub fn configuration_updated(
    window: Window,
    old_configuration: UserSettings,
    new_configuration: UserSettings,
) {
    let diff = old_configuration.diff(&new_configuration);

    if diff.disable_autolaunch.is_some() {
        update_auto_launch(&new_configuration);
    }

    if diff.shortcut.is_some() {
        register_global_shortcut(&window, &window.app_handle(), &new_configuration);
        if let Err(error) = window
            .app_handle()
            .tray_handle()
            .set_menu(menu::get_tray_menu(
                window.app_handle().package_info(),
                &new_configuration,
            ))
        {
            log::error!("Error updating system tray {:?}", error);
        }
    }
}

// Helper used to update the global shortcut
fn register_global_shortcut(window: &Window, app_handle: &AppHandle, settings: &UserSettings) {
    // Register global shortcut
    let mut shortcuts = app_handle.global_shortcut_manager();
    if let Err(error) = shortcuts.unregister_all() {
        log::info!("Unable to unregister all shortcuts {:?}", error);
    }

    match shortcuts.is_registered(&settings.shortcut) {
        Ok(is_registered) => {
            if !is_registered {
                log::info!("Registering {} as shortcut", &settings.shortcut);
                let app_hand = app_handle.clone();
                if let Err(e) = shortcuts.register(&settings.shortcut, move || {
                    let window = window::get_searchbar(&app_hand);
                    window::show_search_bar(&window);
                }) {
                    window::alert(window, "Error registering global shortcut", &format!("{e}"));
                }
            }
        }
        Err(e) => {
            window::alert(window, "Error registering global shortcut", &format!("{e}"));
        }
    }
}

// Helper method used to update the auto launch configuration
pub fn update_auto_launch(user_settings: &UserSettings) {
    // Check and register this app to run on boot
    let binary = current_binary(&Env::default());
    if let Ok(path) = binary {
        // NOTE: See how this works: https://github.com/Teamwork/node-auto-launch#how-does-it-work
        if let Ok(auto) = AutoLaunchBuilder::new()
            .set_app_name("Spyglass Search")
            .set_app_path(&path.display().to_string())
            .set_use_launch_agent(true)
            .build()
        {
            if !user_settings.disable_autolaunch && cfg!(not(debug_assertions)) {
                if let Ok(false) = auto.is_enabled() {
                    let _ = auto.enable();
                }
            } else if let Ok(true) = auto.is_enabled() {
                let _ = auto.disable();
            }
        }
    }
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

async fn check_version_interval(current_version: String, app_handle: AppHandle) {
    let mut interval =
        tokio::time::interval(Duration::from_secs(constants::VERSION_CHECK_INTERVAL_S));

    let shutdown_tx = app_handle.state::<broadcast::Sender<AppEvent>>();
    let mut shutdown = shutdown_tx.subscribe();
    let metrics = app_handle.try_state::<Metrics>();

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                log::info!("🛑 Shutting down version checker");
                return;
            },
            _ = interval.tick() => {
                log::info!("checking for update...");
                if let Some(ref metrics) = metrics {
                    metrics.track(Event::UpdateCheck { current_version: current_version.clone() }).await;
                }

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
