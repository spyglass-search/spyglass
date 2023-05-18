use jsonrpsee::core::{Error, JsonValue};
use jsonrpsee::proc_macros::rpc;
use shared::config::UserSettings;
use shared::request::{BatchDocumentRequest, RawDocumentRequest, SearchLensesParam, SearchParam};
use shared::response::{
    AppStatus, DefaultIndices, LensResult, LibraryStats, ListConnectionResult, PluginResult,
    SearchLensesResp, SearchResults,
};
use std::collections::HashMap;

mod events;
pub use events::*;

/// Rpc trait
#[rpc(server, client, namespace = "spyglass")]
pub trait Rpc {
    /// Returns a protocol version
    #[method(name = "protocol_version")]
    fn protocol_version(&self) -> Result<String, Error>;

    #[method(name = "system_health")]
    fn system_health(&self) -> Result<JsonValue, Error>;

    /// Adds an unparsed document to the spyglass index.
    #[method(name = "index.add_raw_document")]
    async fn add_raw_document(&self, doc: RawDocumentRequest) -> Result<(), Error>;

    /// Adds a whole bunch of documents at once to the spyglass index.
    #[method(name = "index.add_document_batch")]
    async fn add_document_batch(&self, req: BatchDocumentRequest) -> Result<(), Error>;

    /// Checks whether a URL has been indexed
    #[method(name = "index.is_document_indexed")]
    async fn is_document_indexed(&self, url: String) -> Result<bool, Error>;

    /// Permanently deletes a document from the spyglass index and any associated
    /// data.
    #[method(name = "index.delete_document")]
    async fn delete_document(&self, id: String) -> Result<(), Error>;

    #[method(name = "index.delete_document_by_url")]
    async fn delete_document_by_url(&self, url: String) -> Result<(), Error>;

    #[method(name = "authorize_connection")]
    async fn authorize_connection(&self, id: String) -> Result<(), Error>;

    #[method(name = "app_status")]
    async fn app_status(&self) -> Result<AppStatus, Error>;

    #[method(name = "default_indices")]
    async fn default_indices(&self) -> Result<DefaultIndices, Error>;

    #[method(name = "get_library_stats")]
    async fn get_library_stats(&self) -> Result<HashMap<String, LibraryStats>, Error>;

    #[method(name = "install_lens")]
    async fn install_lens(&self, lens_name: String) -> Result<(), Error>;

    #[method(name = "list_connections")]
    async fn list_connections(&self) -> Result<ListConnectionResult, Error>;

    #[method(name = "list_installed_lenses")]
    async fn list_installed_lenses(&self) -> Result<Vec<LensResult>, Error>;

    #[method(name = "list_plugins")]
    async fn list_plugins(&self) -> Result<Vec<PluginResult>, Error>;

    #[method(name = "recrawl_domain")]
    async fn recrawl_domain(&self, domain: String) -> Result<(), Error>;

    #[method(name = "resync_connection")]
    async fn resync_connection(&self, id: String, account: String) -> Result<(), Error>;

    #[method(name = "revoke_connection")]
    async fn revoke_connection(&self, id: String, account: String) -> Result<(), Error>;

    #[method(name = "search_docs")]
    async fn search_docs(&self, query: SearchParam) -> Result<SearchResults, Error>;

    #[method(name = "search_lenses")]
    async fn search_lenses(&self, query: SearchLensesParam) -> Result<SearchLensesResp, Error>;

    #[method(name = "update_user_settings")]
    async fn update_user_settings(
        &self,
        user_settings: UserSettings,
    ) -> Result<UserSettings, Error>;

    #[method(name = "user_settings")]
    async fn user_settings(&self) -> Result<UserSettings, Error>;

    #[method(name = "toggle_pause")]
    async fn toggle_pause(&self, is_paused: bool) -> Result<(), Error>;

    #[method(name = "toggle_plugin")]
    async fn toggle_plugin(&self, name: String, enabled: bool) -> Result<(), Error>;

    #[method(name = "uninstall_lens")]
    async fn uninstall_lens(&self, name: String) -> Result<(), Error>;

    #[subscription(name = "subscribe_events", item = RpcEvent)]
    fn subscribe_events(&self, events: Vec<RpcEventType>);
}
