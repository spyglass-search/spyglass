#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use auto_launch::AutoLaunchBuilder;
use constants::SEARCH_WIN_NAME;
use rpc::RpcMutex;
use tauri::image::Image;
use tauri::process::current_binary;
use tauri::tray::TrayIconBuilder;
use tauri::{include_image, AppHandle, Env, Manager, PackageInfo, RunEvent, Window};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::broadcast;
use tokio::time::Duration;
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

use diff::Diff;
use shared::config::{Config, UserSettings};
use shared::metrics::{Event, Metrics};
use spyglass_rpc::RpcClient;

mod cmd;
mod constants;
mod menu;
mod platform;
mod plugins;
mod rpc;
mod window;
use window::show_update_window;

use crate::window::get_searchbar;

const LOG_LEVEL: tracing::Level = tracing::Level::INFO;
#[cfg(not(debug_assertions))]
const SPYGLASS_LEVEL: &str = "spyglass_app=INFO";
#[cfg(debug_assertions)]
const SPYGLASS_LEVEL: &str = "spyglass_app=DEBUG";

const TRAY_ICON: Image<'_> = include_image!("icons/tray-icon.png");

#[derive(Clone)]
pub enum AppEvent {
    BackendConnected,
    Shutdown,
}
type PauseState = AtomicBool;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = tauri::generate_context!();
    let current_version = current_version(ctx.package_info());
    let config = Config::new();

    // #[cfg(not(debug_assertions))]
    // let _guard = if config.user_settings.disable_telemetry {
    //     None
    // } else {
    //     Some(sentry::init((
    //         "https://13d7d51a8293459abd0aba88f99f4c18@o1334159.ingest.sentry.io/6600471",
    //         sentry::ClientOptions {
    //             release: Some(std::borrow::Cow::from(
    //                 ctx.package_info().version.to_string(),
    //             )),
    //             traces_sample_rate: 0.1,
    //             ..Default::default()
    //         },
    //     )))
    // };

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
        .with(fmt::Layer::new().with_ansi(false).with_writer(non_blocking));

    tracing::subscriber::set_global_default(subscriber).expect("Unable to set a global subscriber");
    LogTracer::init()?;

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log::info!("single instance triggered");
            // handle deep-lining with _args
            // app_handle_clone.trigger_any("scheme-request-received", Some(request));
            let _ = app
                .get_webview_window(SEARCH_WIN_NAME)
                .expect("no main window")
                .set_focus();
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let window = window::get_searchbar(app);
                        // `platform::is_visible()` returns `true` on Windows when
                        // the search bar is built, so we cannot really know if the
                        // window is visible when the `close_search_bar` setting is used.
                        if platform::is_visible(&window) {
                            window::hide_search_bar(&window)
                        } else {
                            window::show_search_bar(&window);
                        }
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
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
            cmd::load_action_settings,
            cmd::load_user_settings,
            cmd::navigate,
            cmd::network_change,
            cmd::open_folder_path,
            cmd::open_lens_folder,
            cmd::open_result,
            cmd::open_settings_folder,
            cmd::recrawl_domain,
            cmd::resize_window,
            cmd::resync_connection,
            cmd::revoke_connection,
            cmd::save_user_settings,
            cmd::search_docs,
            cmd::search_lenses,
            cmd::update_and_restart,
            cmd::wizard_finished,
        ])
        .menu(menu::get_app_menu)
        .setup(move |app| {
            let app_handle = app.app_handle();
            app.manage(config.clone());
            app.manage(Arc::new(PauseState::new(false)));
            app.manage(shared::metrics::Metrics::new(
                &Config::machine_identifier(),
                config.user_settings.disable_telemetry,
            ));

            let (appevent_channel, _) = broadcast::channel::<AppEvent>(1);
            app.manage(appevent_channel);

            let _ = window::show_startup_window(app_handle);

            let config = Config::new();
            log::info!("Loading prefs from: {:?}", Config::prefs_dir());

            log::info!("building tray icon");
            let _ = TrayIconBuilder::with_id("main-tray")
                .menu(&menu::get_tray_menu(
                    app_handle,
                    app.package_info(),
                    &config.user_settings,
                )?)
                .icon(TRAY_ICON)
                .icon_as_template(true)
                .menu_on_left_click(true)
                .on_menu_event(menu::handle_tray_menu_events)
                .on_tray_icon_event(menu::handle_tray_icon_events)
                .build(app)?;

            // Copy default plugins to data directory to be picked up by the backend
            // if let Err(e) = copy_plugins(&config, app.path()) {
            //     log::error!("Unable to copy default plugins: {}", e);
            // }

            let bar = get_searchbar(app_handle);
            bar.show()?;

            if let Err(err) = register_global_shortcut(app_handle, &config.user_settings) {
                dbg!(err);
            }

            // Spawn a version checking background task. Check every couple hours
            // for a new version.
            tauri::async_runtime::spawn(check_version_interval(
                current_version,
                app_handle.clone(),
            ));

            Ok(())
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

/// Handle custom scheme requests (spyglass:// urls).
/// TODO: readd support for app URLs
#[allow(dead_code)]
async fn on_custom_scheme_request(app_handle: AppHandle, event: tauri::Event) {
    if let Ok(request) = url::Url::parse(event.payload()) {
        log::debug!("Received custom uri request: {}", &request);
        // Parse the command from the request
        let event = request.domain().unwrap_or_default();
        let command = request.path();
        let args = request
            .query_pairs()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<HashMap<String, String>>();

        // Only really one event right now but gives us room to grow.
        if event == "command" && command == "/install-lens" {
            if let Some(lens_name) = args.get("name") {
                log::info!("installing lens from app url: {}", lens_name);
                let _ = window::notify(&app_handle, "Spyglass", "Installing lens...");

                // track stuff if metrics is enabled
                if let Some(metrics) = app_handle.try_state::<Metrics>() {
                    metrics
                        .track(Event::InstallLensFromUrl {
                            lens: lens_name.clone(),
                        })
                        .await;
                }

                let _ = crate::plugins::lens_updater::handle_install_lens(
                    &app_handle,
                    lens_name,
                    false,
                )
                .await;
            }
        }

        log::debug!("parsed: {} - {} - {:?}", event, command, args);
    }
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
        let _ = register_global_shortcut(window.app_handle(), &new_configuration);
    }
}

// Helper method used to access the current application version
pub fn current_version(pkg_info: &PackageInfo) -> String {
    format!("v20{}", pkg_info.version)
}

// Helper used to update the global shortcut
fn register_global_shortcut(app_handle: &AppHandle, settings: &UserSettings) -> anyhow::Result<()> {
    // Register global shortcut
    let shortcuts = app_handle.global_shortcut();
    if let Err(error) = shortcuts.unregister_all() {
        log::warn!("Unable to unregister all shortcuts {}", error.to_string());
    }

    let hotkey = Shortcut::from_str(&settings.shortcut)?;
    if !shortcuts.is_registered(hotkey) {
        log::info!("Registering {} as shortcut", &settings.shortcut);
        shortcuts.register(hotkey)?;
    }

    Ok(())
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

async fn pause_crawler(app: AppHandle, _menu_id: String) {
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

                let _new_label = if is_paused {
                    "▶️ Resume indexing"
                } else {
                    "⏸ Pause indexing"
                };

                // if let Some(tray) = app.tray_by_id("main-tray"){
                //     let _ = item_handle.set_title(new_label);
                //     let _ = item_handle.set_enabled(true);
                // }
            }
            Err(err) => log::warn!("Error sending RPC: {}", err),
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

                if let Ok(Some(_)) = app_handle.updater().expect("Unable to get updater").check().await {
                    // show update dialog
                    show_update_window(&app_handle);
                }
            }
        }
    }
}
