use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};

use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use tauri::api::process::{Command, CommandEvent};
use tauri::async_runtime::JoinHandle;
use tokio::signal;
use tokio::sync::Mutex;
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::Retry;

use shared::config::Config;
use spyglass_rpc::RpcClient;

pub type RpcMutex = Arc<Mutex<SpyglassServerClient>>;

pub struct SpyglassServerClient {
    pub client: HttpClient,
    pub endpoint: String,
    pub sidecar_handle: Option<JoinHandle<()>>,
    pub restarts: AtomicU8,
}

async fn connect(endpoint: &str) -> anyhow::Result<HttpClient> {
    match HttpClientBuilder::default().build(endpoint) {
        Ok(client) => {
            // Wait until we have a connection
            let retry_strategy = ExponentialBackoff::from_millis(100).take(3);
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

async fn try_connect(endpoint: &str) -> anyhow::Result<HttpClient> {
    let retry_strategy = ExponentialBackoff::from_millis(10).take(10);
    Retry::spawn(retry_strategy, || connect(endpoint)).await
}

impl SpyglassServerClient {
    pub async fn daemon_eyes(rpc: RpcMutex) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = signal::ctrl_c() => {
                    let rpc = rpc.lock().await;
                    if let Some(handle) = &rpc.sidecar_handle {
                        handle.abort();
                    }
                    break;
                },
                _ = interval.tick() => {
                    let mut rpc = rpc.lock().await;
                    if let Err(err) = rpc.client.protocol_version().await {
                        log::error!("rpc health check error: {}, restart #: {}", err, rpc.restarts.load(Ordering::Relaxed));
                        rpc.reconnect().await;
                        rpc.restarts.fetch_add(1, Ordering::SeqCst);
                        log::info!("restarted");
                    }
                }
            }
        }
    }

    pub async fn new(config: &Config) -> Self {
        let endpoint = format!("http://127.0.0.1:{}", config.user_settings.port);
        log::info!("Connecting to backend @ {}", &endpoint);

        // Only startup & manage sidecar in release mode.
        #[cfg(not(debug_assertions))]
        let sidecar_handle = Some(SpyglassServerClient::check_and_start_backend());

        log::info!("backend started");
        let client = try_connect(&endpoint)
            .await
            .expect("Unable to connect to spyglass backend!");

        #[cfg(debug_assertions)]
        let sidecar_handle = None;

        SpyglassServerClient {
            client,
            endpoint: endpoint.clone(),
            sidecar_handle,
            restarts: AtomicU8::new(0),
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
        self.client = try_connect(&self.endpoint)
            .await
            .expect("Unable to connect to spyglass backend!");
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
                        sentry::capture_error(&std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            format!("sidecar terminated: {:?}", payload),
                        ));
                        log::error!("sidecar terminated: {:?}", payload);
                        return;
                    }
                    _ => {}
                }
            }
        })
    }
}
