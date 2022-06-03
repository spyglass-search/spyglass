use std::sync::Arc;

use jsonrpc_core_client::{transports::ipc, TypedClient};
use serde::de::DeserializeOwned;
use serde::Serialize;
use shared::rpc::gen_ipc_path;
use tauri::api::process::{Command, CommandEvent};
use tokio::sync::Mutex;
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;

pub type RpcMutex = Arc<Mutex<RpcClient>>;

pub struct RpcClient {
    pub client: TypedClient,
    pub endpoint: String,
}

pub fn check_and_start_backend() {
    tauri::async_runtime::spawn(async move {
        let (mut rx, _) = Command::new_sidecar("spyglass-server")
            .expect("failed to create `spyglass-server` binary command")
            .spawn()
            .expect("Failed to spawn sidecar");

        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Error(line) => log::error!("sidecar error: {}", line),
                CommandEvent::Terminated(payload) => {
                    log::error!("sidecar terminated: {:?}", payload)
                }
                _ => {}
            }
        }
    });
}

async fn connect(endpoint: &str) -> Result<TypedClient, ()> {
    if let Ok(client) = ipc::connect(endpoint).await {
        return Ok(client);
    }

    Err(())
}

async fn try_connect(endpoint: &str) -> Result<TypedClient, ()> {
    let retry_strategy = ExponentialBackoff::from_millis(10)
        .map(jitter) // add jitter to delays
        .take(10);

    Retry::spawn(retry_strategy, || connect(endpoint)).await
}

impl RpcClient {
    pub async fn new() -> Self {
        let endpoint = gen_ipc_path();

        let client = try_connect(&endpoint)
            .await
            .expect("Unable to connect to spyglass backend!");

        RpcClient {
            client,
            endpoint: endpoint.clone(),
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
                log::error!("Error sending RPC: {}", err);
                self.reconnect().await;
                R::default()
            }
        }
    }

    pub async fn reconnect(&mut self) {
        log::info!("Attempting to restart backend");
        // Attempt to reconnect
        check_and_start_backend();
        self.client = try_connect(&self.endpoint)
            .await
            .expect("Unable to connect to spyglass backend!");
        log::info!("restarted");
    }
}
