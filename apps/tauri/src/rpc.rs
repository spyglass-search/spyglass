use jsonrpsee::ws_client::{WsClient, WsClientBuilder};
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};
use tauri::async_runtime::JoinHandle;
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_shell::{process::CommandEvent, ShellExt};
use tokio::sync::{broadcast, Mutex};
use tokio_retry::strategy::FixedInterval;
use tokio_retry::Retry;

use shared::config::Config;
use spyglass_rpc::RpcClient;

use crate::AppEvent;

pub type RpcMutex = Arc<Mutex<SpyglassServerClient>>;

pub struct SpyglassServerClient {
    pub client: WsClient,
    pub endpoint: String,
    pub sidecar_handle: Option<JoinHandle<()>>,
    pub restarts: AtomicU8,
    pub app_handle: AppHandle,
}

/// Build client & attempt a connection to the health check endpoint.
async fn try_connect(endpoint: &str) -> anyhow::Result<WsClient> {
    log::info!("connecting to backend via {}", endpoint);
    // Wait until we have a connection
    let retry_strategy = FixedInterval::from_millis(5000).take(4);
    match Retry::spawn(retry_strategy, || {
        WsClientBuilder::default()
            .connection_timeout(std::time::Duration::from_secs(10))
            .request_timeout(std::time::Duration::from_secs(10))
            .build(endpoint)
    })
    .await
    {
        Ok(client) => {
            let v = client.protocol_version().await;
            log::info!("connected to daemon w/ version: {:?}", v);
            Ok(client)
        }
        Err(e) => {
            log::warn!("error connecting: {:?}", e);
            Err(anyhow::anyhow!(e.to_string()))
        }
    }
}

impl SpyglassServerClient {
    /// Monitors the health of the backend & recreates it necessary.
    pub async fn daemon_eyes(rpc: RpcMutex, mut shutdown: broadcast::Receiver<AppEvent>) {
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
        let endpoint = format!("ws://127.0.0.1:{}", config.user_settings.port);
        log::info!("Connecting to backend @ {}", &endpoint);

        // Only startup & manage sidecar in release mode.
        #[cfg(not(debug_assertions))]
        let sidecar_handle = Some(SpyglassServerClient::check_and_start_backend(app_handle));

        let client = match try_connect(&endpoint).await {
            Ok(client) => Some(client),
            Err(err) => {
                // Let users know something has gone dreadfully wrong.
                app_handle
                    .dialog()
                    .message(format!(
                        "Error: {err}\nPlease file a bug report!\nThe application will exit now."
                    ))
                    .title("Unable to start search backend")
                    .kind(MessageDialogKind::Error);

                app_handle.exit(0);
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
            sidecar.abort();

            log::info!("Attempting to restart backend");
            self.sidecar_handle = Some(SpyglassServerClient::check_and_start_backend(
                &self.app_handle,
            ));
        }

        log::info!("reconnecting to {}", self.endpoint);
        match try_connect(&self.endpoint).await {
            Ok(client) => {
                self.client = client;
            }
            Err(err) => {
                // Let users know something has gone dreadfully wrong.
                self.app_handle
                    .dialog()
                    .message(format!(
                        "Error: {}\nPlease file a bug report!\nThe application will exit now.",
                        &err.to_string()
                    ))
                    .title("Unable to start search backend")
                    .kind(MessageDialogKind::Error);
            }
        }
    }

    pub fn check_and_start_backend(app: &AppHandle) -> JoinHandle<()> {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let app = app.clone();
            let shell = app.shell();
            let (mut rx, _) = shell
                .sidecar("spyglass-server")
                .expect("failed to create `spyglass-server` binary command")
                .spawn()
                .expect("failed to spawn sidecar");

            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Error(message) => {
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
