use jsonrpsee::core::Error;
use jsonrpsee::proc_macros::rpc;

use crate::request::{SearchLensesParam, SearchParam};
use crate::response::{
    AppStatus, CrawlStats, LensResult, PluginResult, SearchLensesResp, SearchResults,
};

pub fn gen_ipc_path() -> String {
    if cfg!(windows) {
        r"\\.\pipe\ipc-spyglass".to_string()
    } else {
        r"/tmp/ipc-spyglass".to_string()
    }
}

/// Rpc trait
#[rpc(server, client, namespace = "state")]
pub trait Rpc {
    /// Returns a protocol version
    #[method(name = "protocol_version")]
    fn protocol_version(&self) -> Result<String, Error>;

    #[method(name = "app_status")]
    async fn app_status(&self) -> Result<AppStatus, Error>;

    #[method(name = "crawl_stats")]
    async fn crawl_stats(&self) -> Result<CrawlStats, Error>;

    #[method(name = "delete_doc")]
    async fn delete_doc(&self, id: String) -> Result<(), Error>;

    #[method(name = "delete_domain")]
    async fn delete_domain(&self, domain: String) -> Result<(), Error>;

    #[method(name = "list_installed_lenses")]
    async fn list_installed_lenses(&self) -> Result<Vec<LensResult>, Error>;

    #[method(name = "list_plugins")]
    async fn list_plugins(&self) -> Result<Vec<PluginResult>, Error>;

    #[method(name = "recrawl_domain")]
    async fn recrawl_domain(&self, domain: String) -> Result<(), Error>;

    #[method(name = "search_docs")]
    async fn search_docs(&self, query: SearchParam) -> Result<SearchResults, Error>;

    #[method(name = "search_lenses")]
    async fn search_lenses(&self, query: SearchLensesParam) -> Result<SearchLensesResp, Error>;

    #[method(name = "toggle_pause")]
    async fn toggle_pause(&self, is_paused: bool) -> Result<(), Error>;

    #[method(name = "toggle_plugin")]
    async fn toggle_plugin(&self, name: String) -> Result<(), Error>;
}
