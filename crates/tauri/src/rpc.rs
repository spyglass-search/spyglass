use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};

use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use tauri::api::dialog::blocking::message;
use tauri::async_runtime::JoinHandle;
use tauri::{
    api::process::{Command, CommandEvent},
    AppHandle, Manager,
};
use tokio::sync::{broadcast, Mutex};
use tokio_retry::strategy::FixedInterval;
use tokio_retry::Retry;

use shared::config::Config;
use spyglass_rpc::RpcClient;

use crate::{constants, AppShutdown};

pub type RpcMutex = Arc<Mutex<SpyglassServerClient>>;

pub struct SpyglassServerClient {
    pub client: HttpClient,
    pub endpoint: String,
    pub sidecar_handle: Option<JoinHandle<()>>,
    pub restarts: AtomicU8,
    pub app_handle: AppHandle,
}

/// Build client & attempt a connection to the health check endpoint.
async fn try_connect(endpoint: &str) -> anyhow::Result<HttpClient> {
    match HttpClientBuilder::default()
        .request_timeout(std::time::Duration::from_secs(30))
        .build(endpoint)
    {
        Ok(client) => {
            // Wait until we have a connection
            let retry_strategy = FixedInterval::from_millis(100).take(4);
            match Retry::spawn(retry_strategy, || client.protocol_version()).await {
                Ok(v) => {
                    log::info!("connected to daemon w/ version: {}", v);
                    Ok(client)
                }
                Err(err) => Err(anyhow::anyhow!(err.to_string())),
            }
        }
        Err(e) => {
            sentry::capture_error(&e);
            Err(anyhow::anyhow!(e.to_string()))
        }
    }
}

impl SpyglassServerClient {
    /// Monitors the health of the backend & recreates it necessary.
    pub async fn daemon_eyes(rpc: RpcMutex, mut shutdown: broadcast::Receiver<AppShutdown>) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    log::info!("🛑 Shutting down sidecar");

                    let rpc = rpc.lock().await;
                    if let Some(handle) = &rpc.sidecar_handle {
                        handle.abort();
                    }

                    return;
                },
                _ = interval.tick() => {
                    let mut rpc = rpc.lock().await;
                    if let Err(err) = rpc.client.protocol_version().await {
                        // Keep track of the number restarts
                        let num_restarts = rpc.restarts.fetch_add(1, Ordering::SeqCst);
                        log::error!("rpc health check error: {}, restart #: {}", err, num_restarts);
                        rpc.reconnect().await;
                        log::info!("restarted");
                    }
                }
            }
        }
    }

    pub async fn new(config: &Config, app_handle: &AppHandle) -> Self {
        let endpoint = format!("http://127.0.0.1:{}", config.user_settings.port);
        log::info!("Connecting to backend @ {}", &endpoint);

        // Only startup & manage sidecar in release mode.
        #[cfg(not(debug_assertions))]
        let sidecar_handle = Some(SpyglassServerClient::check_and_start_backend());

        log::info!("backend started");
        let client = match try_connect(&endpoint).await {
            Ok(client) => Some(client),
            Err(err) => {
                if let Some(window) = app_handle.get_window(constants::SEARCH_WIN_NAME) {
                    // Let users know something has gone dreadfully wrong.
                    message(
                        Some(&window),
                        "Unable to start search backend",
                        format!(
                            "Error: {}\nPlease file a bug report!\nThe application will exit now.",
                            &err.to_string()
                        ),
                    );

                    app_handle.exit(0);
                }

                None
            }
        };

        #[cfg(debug_assertions)]
        let sidecar_handle = None;

        SpyglassServerClient {
            client: client.expect("Unable to create search client"),
            endpoint: endpoint.clone(),
            sidecar_handle,
            restarts: AtomicU8::new(0),
            app_handle: app_handle.clone(),
        }
    }

    pub async fn reconnect(&mut self) {
        // Attempt to reconnect
        if let Some(sidecar) = &self.sidecar_handle {
            log::info!("child process killed");
            tauri::api::process::kill_children();
            sidecar.abort();

            log::info!("Attempting to restart backend");
            self.sidecar_handle = Some(SpyglassServerClient::check_and_start_backend());
        }

        log::info!("reconnecting to {}", self.endpoint);
        match try_connect(&self.endpoint).await {
            Ok(client) => {
                self.client = client;
            }
            Err(err) => {
                if let Some(window) = self.app_handle.get_window(constants::SEARCH_WIN_NAME) {
                    // Let users know something has gone dreadfully wrong.
                    message(
                        Some(&window),
                        "Unable to start search backend",
                        format!(
                            "Error: {}\nPlease file a bug report!\nThe application will exit now.",
                            &err.to_string()
                        ),
                    );
                }
            }
        }
    }

    pub fn check_and_start_backend() -> JoinHandle<()> {
        tauri::async_runtime::spawn(async move {
            let (mut rx, _) = Command::new_sidecar("spyglass-server")
                .expect("failed to create `spyglass-server` binary command")
                .spawn()
                .expect("Failed to spawn sidecar");

            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Error(message) => {
                        sentry::capture_error(&std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            message.clone(),
                        ));
                        log::error!("sidecar error: {}", message);
                        return;
                    }
                    CommandEvent::Terminated(payload) => {
                        log::error!("sidecar terminated: {:?}", payload);
                        return;
                    }
                    _ => {}
                }
            }
        })
    }
}
