use jsonrpsee_core::client::ClientT;
use jsonrpsee_core::rpc_params;
use jsonrpsee_wasm_client::{Client, WasmClientBuilder};
use reqwest::Client as HttpClient;
use shared::request::SearchParam;
use shared::response::{SearchResult, SearchResults};
use thiserror::Error;

use crate::constants;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("No available clients")]
    NoAvailableClients,
    #[error("HTTP request error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("RPC request error: {0}")]
    RpcError(#[from] jsonrpsee_core::Error),
}

#[derive(Debug)]
pub struct SpyglassClient {
    /// used for web socket connections
    // todo: detect disconnections & handle reconnection
    rpc_client: Option<Client>,
    /// used for http only connections
    http_client: Option<HttpClient>,
}

impl SpyglassClient {
    pub async fn ws_client() -> Self {
        let client = WasmClientBuilder::default()
            .request_timeout(std::time::Duration::from_secs(10))
            .build(constants::RPC_ENDPOINT)
            .await
            .expect("Unable to create WsClient");

        Self {
            rpc_client: Some(client),
            http_client: None,
        }
    }

    pub async fn http_client() -> Self {
        let client = HttpClient::new();

        Self {
            rpc_client: None,
            http_client: Some(client),
        }
    }

    pub async fn search(&mut self, query: &str) -> Result<Vec<SearchResult>, ClientError> {
        if let Some(client) = &self.http_client {
            let url = format!("{}/search", constants::HTTP_ENDPOINT);
            let res = client.get(url).query(&[("query", query)]).send().await?;
            Ok(res.json().await?)
        } else if let Some(client) = &self.rpc_client {
            let params = SearchParam {
                lenses: Vec::new(),
                query: query.to_string(),
            };

            let resp = client
                .request::<SearchResults, _>("spyglass_search_docs", rpc_params![params])
                .await?;
            Ok(resp.results)
        } else {
            Err(ClientError::NoAvailableClients)
        }
    }
}
