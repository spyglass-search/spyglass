use jsonrpc_core_client::{transports::ipc, TypedClient};
use shared::rpc::gen_ipc_path;

pub struct RpcClient {
    pub client: TypedClient,
    pub endpoint: String,
}

impl RpcClient {
    pub async fn new() -> Self {
        let endpoint = gen_ipc_path();

        let client: TypedClient = ipc::connect(endpoint.clone()).await.unwrap();

        RpcClient {
            client,
            endpoint: endpoint.clone(),
        }
    }
}
