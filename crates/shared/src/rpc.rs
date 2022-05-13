use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_derive::rpc;

use crate::request::{SearchLensesParam, SearchParam};
use crate::response::{AppStatus, SearchLensesResp, SearchResults};

pub fn gen_ipc_path() -> String {
    if cfg!(windows) {
        r"\\.\pipe\ipc-spyglass".to_string()
    } else {
        r"/tmp/ipc-spyglass".to_string()
    }
}

/// Rpc trait
#[rpc]
pub trait Rpc {
    /// Returns a protocol version
    #[rpc(name = "protocol_version")]
    fn protocol_version(&self) -> Result<String>;

    #[rpc(name = "app_status")]
    fn app_status(&self) -> BoxFuture<Result<AppStatus>>;

    #[rpc(name = "toggle_pause")]
    fn toggle_pause(&self) -> BoxFuture<Result<AppStatus>>;

    #[rpc(name = "search_docs")]
    fn search_docs(&self, query: SearchParam) -> BoxFuture<Result<SearchResults>>;

    #[rpc(name = "search_lenses")]
    fn search_lenses(&self, query: SearchLensesParam) -> BoxFuture<Result<SearchLensesResp>>;
}
