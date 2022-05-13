use jsonrpc_core_client::{transports::ipc, TypedClient};
use shared::rpc::gen_ipc_path;
use tauri::api::process::Command;
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;

pub struct RpcClient {
    pub client: TypedClient,
    pub endpoint: String,
}

pub fn check_and_start_backend() {
    let _ = Command::new_sidecar("spyglass-server")
        .expect("failed to create `spyglass-server` binary command")
        .spawn()
        .expect("Failed to spawn sidecar");
}

async fn connect(endpoint: String) -> Result<TypedClient, ()> {
    if let Ok(client) = ipc::connect(endpoint.clone()).await {
        return Ok(client);
    }

    Err(())
}

async fn try_connect(endpoint: &String) -> Result<TypedClient, ()> {
    let retry_strategy = ExponentialBackoff::from_millis(10)
        .map(jitter) // add jitter to delays
        .take(10);

    Retry::spawn(retry_strategy, || connect(endpoint.clone()))
        .await
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

    pub async fn reconnect(&mut self) {
        log::info!("Attempting to restart backend");
        // Attempt to reconnect
        check_and_start_backend();
        self.client = try_connect(&self.endpoint)
            .await
            .expect("Unable to connect to spyglass backend!");
    }
}
