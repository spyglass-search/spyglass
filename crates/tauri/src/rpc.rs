use std::sync::Arc;

use jsonrpc_core_client::{transports::ipc, TypedClient};
use serde::de::DeserializeOwned;
use serde::Serialize;
use shared::rpc::gen_ipc_path;
use tauri::api::process::{Command, CommandEvent};
use tauri::async_runtime::JoinHandle;
use tokio::sync::Mutex;
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::Retry;

pub type RpcMutex = Arc<Mutex<RpcClient>>;

pub struct RpcClient {
    pub client: TypedClient,
    pub endpoint: String,
    pub sidecar_handle: Option<JoinHandle<()>>,
}

async fn connect(endpoint: &str) -> Result<TypedClient, ()> {
    match ipc::connect(endpoint).await {
        Ok(client) => Ok(client),
        Err(e) => {
            sentry::capture_error(&e);
            Err(())
        }
    }
}

async fn try_connect(endpoint: &str) -> Result<TypedClient, ()> {
    let retry_strategy = ExponentialBackoff::from_millis(10).take(10);

    Retry::spawn(retry_strategy, || connect(endpoint)).await
}

impl RpcClient {
    pub async fn new() -> Self {
        let endpoint = gen_ipc_path();
        log::info!("Starting backend");

        // Only startup & manage sidecar in release mode.
        #[cfg(not(debug_assertions))]
        let sidecar_handle = Some(RpcClient::check_and_start_backend());

        let client = try_connect(&endpoint)
            .await
            .expect("Unable to connect to spyglass backend!");

        #[cfg(debug_assertions)]
        let sidecar_handle = None;

        RpcClient {
            client,
            endpoint: endpoint.clone(),
            sidecar_handle,
        }
    }

    pub async fn call<T: Serialize, R: DeserializeOwned + Default>(
        &mut self,
        method: &str,
        args: T,
    ) -> R {
        match self.client.call_method::<T, R>(method, "", args).await {
            Ok(resp) => resp,
            Err(err) => {
                sentry::capture_error(&err);
                log::error!("Error sending RPC: {}", err);
                self.reconnect().await;
                R::default()
            }
        }
    }

    pub async fn reconnect(&mut self) {
        // Attempt to reconnect
        if let Some(sidecar) = &self.sidecar_handle {
            log::info!("child process killed");
            tauri::api::process::kill_children();

            log::info!("Attempting to restart backend");
            sidecar.abort();
            self.sidecar_handle = Some(RpcClient::check_and_start_backend());
        }

        log::info!("Trying to reconnect to backend...");
        self.client = try_connect(&self.endpoint)
            .await
            .expect("Unable to connect to spyglass backend!");
        log::info!("Connected!");
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
