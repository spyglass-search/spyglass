extern crate notify;
use clap::Parser;
use entities::models::{self, crawl_queue, lens};
use libspyglass::pipeline;
use libspyglass::plugin;
use libspyglass::state::AppState;
use libspyglass::task::{self, AppPause, AppShutdown, ManagerCommand};
#[allow(unused_imports)]
use migration::Migrator;
use shared::config::{self, Config};
use std::io;
use tokio::signal;
use tokio::sync::{broadcast, mpsc};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

mod api;

const LOG_LEVEL: tracing::Level = tracing::Level::INFO;
#[cfg(not(debug_assertions))]
const SPYGLASS_LEVEL: &str = "spyglass=INFO";
#[cfg(not(debug_assertions))]
const LIBSPYGLASS_LEVEL: &str = "libspyglass=INFO";

#[cfg(debug_assertions)]
const SPYGLASS_LEVEL: &str = "spyglass=DEBUG";
#[cfg(debug_assertions)]
const LIBSPYGLASS_LEVEL: &str = "libspyglass=DEBUG";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    /// Run migrations & basic checks.
    #[arg(short, long)]
    check: bool,
}

#[cfg(feature = "tokio-console")]
pub fn setup_logging(_config: &Config) {
    let subscriber = tracing_subscriber::registry()
        .with(
            EnvFilter::from_default_env()
                .add_directive("tokio=TRACE".parse().expect("invalid EnvFilter"))
                .add_directive("runtime=TRACE".parse().expect("invalid EnvFilter")),
        )
        .with(
            console_subscriber::ConsoleLayer::builder()
                .with_default_env()
                .spawn(),
        );
    tracing::subscriber::set_global_default(subscriber).expect("Unable to set a global subscriber");
}

#[cfg(not(feature = "tokio-console"))]
pub fn setup_logging(config: &Config) {
    #[cfg(not(debug_assertions))]
    let _guard = if config.user_settings.disable_telemetry {
        None
    } else {
        Some(sentry::init((
            "https://5c1196909a4e4e5689406705be13aad3@o1334159.ingest.sentry.io/6600345",
            sentry::ClientOptions {
                release: sentry::release_name!(),
                traces_sample_rate: 0.1,
                ..Default::default()
            },
        )))
    };

    let file_appender = tracing_appender::rolling::daily(config.logs_dir(), "server.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::registry()
        .with(
            EnvFilter::from_default_env()
                .add_directive(LOG_LEVEL.into())
                .add_directive(SPYGLASS_LEVEL.parse().expect("Invalid EnvFilter"))
                .add_directive(LIBSPYGLASS_LEVEL.parse().expect("Invalid EnvFilter"))
                // Don't need debug/info level logging for these
                .add_directive("tantivy=WARN".parse().expect("Invalid EnvFilter"))
                .add_directive("regalloc=WARN".parse().expect("Invalid EnvFilter"))
                .add_directive("cranelift_codegen=WARN".parse().expect("Invalid EnvFilter"))
                .add_directive("wasmer_wasi=WARN".parse().expect("Invalid EnvFilter"))
                .add_directive(
                    "wasmer_compiler_cranelift=WARN"
                        .parse()
                        .expect("Invalid EnvFilter"),
                )
                .add_directive("docx=WARN".parse().expect("Invalid EnvFilter")),
        )
        .with(fmt::Layer::new().with_writer(io::stdout))
        .with(fmt::Layer::new().with_ansi(false).with_writer(non_blocking))
        .with(sentry_tracing::layer());
    tracing::subscriber::set_global_default(subscriber).expect("Unable to set a global subscriber");
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), ()> {
    let mut config = Config::new();
    let args = CliArgs::parse();

    setup_logging(&config);
    LogTracer::init().expect("Unable to initialize LogTracer");

    log::info!("Loading prefs from: {:?}", Config::prefs_dir());
    // In case we need to split the runtimes for some reason:
    // let num_cores = usize::from(std::thread::available_parallelism().expect("Unable to get number of cores"));
    // let api_rt = tokio::runtime::Builder::new_multi_thread()
    //     .enable_all()
    //     .thread_name("spyglass-api")
    //     .worker_threads((num_cores / 2).max(1))
    //     .build()
    //     .expect("Unable to create tokio runtime");

    // Run any migrations, only on headless mode.
    #[cfg(debug_assertions)]
    {
        use migration::DbErr;
        let migration_status: Result<(), DbErr> = match Migrator::run_migrations().await {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                // This is ok, just the migrator being funky
                if !msg.contains("been applied but its file is missing") {
                    // Ruh-oh something went wrong
                    log::error!("Unable to migrate database - {}", e.to_string());
                    // Exit from app
                    return Err(());
                }

                Ok(())
            }
        };

        if migration_status.is_err() {
            return Ok(());
        }
    }

    {
        // migrate plugin settings
        let db = models::create_connection(&config, false)
            .await
            .expect("Unable to connect to db");

        // state.user_settings
        if let Ok(Some(model)) = lens::find_by_name(config::LEGACY_FILESYSTEM_PLUGIN, &db).await {
            let mut new_settings = config.user_settings.clone();
            new_settings.filesystem_settings.enable_filesystem_scanning = model.is_enabled;
            let _ = Config::save_user_settings(&new_settings);
            if let Ok(settings) = Config::load_user_settings() {
                config.user_settings = settings;
            }

            if let Err(err) = lens::delete_by_id(model.id, &db).await {
                log::error!("Error deleting filesystem plugin lens {:?}", err);
            }
        }
    }

    // Initialize/Load user preferences
    let state = AppState::new(&config).await;
    if !args.check {
        let indexer_handle = start_backend(state.clone(), config.clone());
        // API server
        let api_handle = api::start_api_server(state, config);
        let _ = tokio::join!(indexer_handle, api_handle);
    }

    Ok(())
}

async fn start_backend(state: AppState, config: Config) {
    // Initialize crawl_queue, requeue all in-flight tasks.
    let _ = crawl_queue::reset_processing(&state.db).await;
    if let Err(e) = lens::reset(&state.db).await {
        log::error!("Unable to reset lenses: {}", e);
    }

    // Create channels for scheduler / crawlers
    let (worker_cmd_tx, worker_cmd_rx) = mpsc::channel(
        state
            .user_settings
            .inflight_crawl_limit
            .value()
            .try_into()
            .expect("Unable to parse inflight_crawl_limit"),
    );

    // Channel for pause/unpause listeners
    let (pause_tx, _) = broadcast::channel::<AppPause>(16);

    // Channel for scheduler commands
    let (manager_cmd_tx, manager_cmd_rx) = mpsc::unbounded_channel::<ManagerCommand>();
    // Channel for plugin commands
    let (plugin_cmd_tx, plugin_cmd_rx) = mpsc::channel(16);

    // Channel for pipeline commands
    let (pipeline_cmd_tx, pipeline_cmd_rx) = mpsc::channel(16);

    {
        state
            .manager_cmd_tx
            .lock()
            .await
            .replace(manager_cmd_tx.clone());
    }

    {
        state.pause_cmd_tx.lock().await.replace(pause_tx.clone());
    }

    {
        state
            .plugin_cmd_tx
            .lock()
            .await
            .replace(plugin_cmd_tx.clone());
    }

    {
        state
            .pipeline_cmd_tx
            .lock()
            .await
            .replace(pipeline_cmd_tx.clone());
    }

    // Work scheduler
    let manager_handle = tokio::spawn(task::manager_task(
        state.clone(),
        worker_cmd_tx,
        manager_cmd_tx.clone(),
        manager_cmd_rx,
    ));

    // Crawlers
    let worker_handle = tokio::spawn(task::worker_task(
        state.clone(),
        config.clone(),
        worker_cmd_rx,
        pause_tx.subscribe(),
    ));

    // Check lenses for updates & add any bootstrapped URLs to crawler.
    let lens_watcher_handle = tokio::spawn(task::lens_watcher(
        state.clone(),
        config.clone(),
        pause_tx.subscribe(),
    ));

    // Loads and processes pipeline commands
    let _pipeline_handler = tokio::spawn(pipeline::initialize_pipelines(
        state.clone(),
        config.clone(),
        pipeline_cmd_rx,
    ));

    let watcher = libspyglass::filesystem::SpyglassFileWatcher::new(&state);
    {
        state.file_watcher.lock().await.replace(watcher);
    }

    // Plugin server
    let pm_handle = tokio::spawn(plugin::plugin_event_loop(
        state.clone(),
        config.clone(),
        plugin_cmd_tx.clone(),
        plugin_cmd_rx,
    ));

    // Gracefully handle shutdowns
    match signal::ctrl_c().await {
        Ok(()) => {
            log::warn!("Shutdown request received");
            if let Some(fs) = state.file_watcher.lock().await.as_ref() {
                fs.watcher_handle.abort();
            }

            state
                .shutdown_cmd_tx
                .lock()
                .await
                .send(AppShutdown::Now)
                .expect("Unable to send AppShutdown cmd");
        }
        Err(err) => {
            log::error!("Unable to listen for shutdown signal: {}", err);
            state
                .shutdown_cmd_tx
                .lock()
                .await
                .send(AppShutdown::Now)
                .expect("Unable to send AppShutdown cmd");
        }
    }

    let _ = tokio::join!(
        manager_handle,
        worker_handle,
        pm_handle,
        lens_watcher_handle
    );
}
