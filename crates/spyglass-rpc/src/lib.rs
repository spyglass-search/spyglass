use jsonrpsee::core::Error;
use jsonrpsee::proc_macros::rpc;
use std::collections::HashMap;

use shared::request::{SearchLensesParam, SearchParam};
use shared::response::{
    AppStatus, LensResult, LibraryStats, ListConnectionResult, PluginResult, SearchLensesResp,
    SearchResults,
};

/// Rpc trait
#[rpc(server, client, namespace = "state")]
pub trait Rpc {
    /// Returns a protocol version
    #[method(name = "protocol_version")]
    fn protocol_version(&self) -> Result<String, Error>;

    #[method(name = "authorize_connection")]
    async fn authorize_connection(&self, id: String) -> Result<(), Error>;

    #[method(name = "app_status")]
    async fn app_status(&self) -> Result<AppStatus, Error>;

    #[method(name = "delete_doc")]
    async fn delete_doc(&self, id: String) -> Result<(), Error>;

    #[method(name = "delete_domain")]
    async fn delete_domain(&self, domain: String) -> Result<(), Error>;

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

    #[method(name = "toggle_pause")]
    async fn toggle_pause(&self, is_paused: bool) -> Result<(), Error>;

    #[method(name = "toggle_plugin")]
    async fn toggle_plugin(&self, name: String, enabled: bool) -> Result<(), Error>;

    #[method(name = "uninstall_lens")]
    async fn uninstall_lens(&self, name: String) -> Result<(), Error>;
}
