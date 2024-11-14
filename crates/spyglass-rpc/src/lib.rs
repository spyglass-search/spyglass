use jsonrpsee::core::{JsonValue, RpcResult, SubscriptionResult};
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::{ErrorObject, ErrorObjectOwned};
use serde::Serialize;
use shared::config::UserSettings;
use shared::request::{BatchDocumentRequest, RawDocumentRequest, SearchLensesParam, SearchParam};
use shared::response::{
    AppStatus, DefaultIndices, LensResult, LibraryStats, ListConnectionResult, PluginResult,
    SearchLensesResp, SearchResults,
};
use std::collections::HashMap;

mod events;
pub use events::*;

#[derive(Serialize)]
pub struct ErrorData;

pub fn server_error(msg: String, data: Option<ErrorData>) -> ErrorObjectOwned {
    ErrorObject::owned(500, msg, data)
}

/// Rpc trait
#[rpc(server, client, namespace = "spyglass")]
pub trait Rpc {
    /// Returns a protocol version
    #[method(name = "protocol_version")]
    fn protocol_version(&self) -> RpcResult<String>;

    #[method(name = "system_health")]
    fn system_health(&self) -> RpcResult<JsonValue>;

    /// Adds an unparsed document to the spyglass index.
    #[method(name = "index.add_raw_document")]
    async fn add_raw_document(&self, doc: RawDocumentRequest) -> RpcResult<()>;

    /// Adds a whole bunch of documents at once to the spyglass index.
    #[method(name = "index.add_document_batch")]
    async fn add_document_batch(&self, req: BatchDocumentRequest) -> RpcResult<()>;

    /// Checks whether a URL has been indexed
    #[method(name = "index.is_document_indexed")]
    async fn is_document_indexed(&self, url: String) -> RpcResult<bool>;

    /// Permanently deletes a document from the spyglass index and any associated
    /// data.
    #[method(name = "index.delete_document")]
    async fn delete_document(&self, id: String) -> RpcResult<()>;

    #[method(name = "index.delete_document_by_url")]
    async fn delete_document_by_url(&self, url: String) -> RpcResult<()>;

    #[method(name = "authorize_connection")]
    async fn authorize_connection(&self, id: String) -> RpcResult<()>;

    #[method(name = "app_status")]
    async fn app_status(&self) -> RpcResult<AppStatus>;

    #[method(name = "default_indices")]
    async fn default_indices(&self) -> RpcResult<DefaultIndices>;

    #[method(name = "get_library_stats")]
    async fn get_library_stats(&self) -> RpcResult<HashMap<String, LibraryStats>>;

    #[method(name = "install_lens")]
    async fn install_lens(&self, lens_name: String) -> RpcResult<()>;

    #[method(name = "list_connections")]
    async fn list_connections(&self) -> RpcResult<ListConnectionResult>;

    #[method(name = "list_installed_lenses")]
    async fn list_installed_lenses(&self) -> RpcResult<Vec<LensResult>>;

    #[method(name = "list_plugins")]
    async fn list_plugins(&self) -> RpcResult<Vec<PluginResult>>;

    #[method(name = "recrawl_domain")]
    async fn recrawl_domain(&self, domain: String) -> RpcResult<()>;

    #[method(name = "resync_connection")]
    async fn resync_connection(&self, id: String, account: String) -> RpcResult<()>;

    #[method(name = "revoke_connection")]
    async fn revoke_connection(&self, id: String, account: String) -> RpcResult<()>;

    #[method(name = "search_docs")]
    async fn search_docs(&self, query: SearchParam) -> RpcResult<SearchResults>;

    #[method(name = "search_lenses")]
    async fn search_lenses(&self, query: SearchLensesParam) -> RpcResult<SearchLensesResp>;

    #[method(name = "update_user_settings")]
    async fn update_user_settings(&self, user_settings: UserSettings) -> RpcResult<UserSettings>;

    #[method(name = "user_settings")]
    async fn user_settings(&self) -> RpcResult<UserSettings>;

    #[method(name = "toggle_pause")]
    async fn toggle_pause(&self, is_paused: bool) -> RpcResult<()>;

    #[method(name = "uninstall_lens")]
    async fn uninstall_lens(&self, name: String) -> RpcResult<()>;

    #[subscription(name = "subscribe_events", item = RpcEvent)]
    async fn subscribe_events(&self, events: Vec<RpcEventType>) -> SubscriptionResult;
}
