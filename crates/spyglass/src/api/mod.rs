extern crate jsonrpc_ipc_server;

use jsonrpc_core::{BoxFuture, IoHandler, Result};
use jsonrpc_ipc_server::{Server, ServerBuilder};

use libspyglass::state::AppState;

use shared::request::{SearchLensesParam, SearchParam};
use shared::response::{AppStatus, SearchLensesResp, SearchResults};
use shared::rpc::{gen_ipc_path, Rpc};

mod response;
mod route;

pub struct SpyglassRPC {
    state: AppState,
}

impl Rpc for SpyglassRPC {
    fn protocol_version(&self) -> Result<String> {
        Ok("version1".into())
    }

    fn app_status(&self) -> BoxFuture<Result<AppStatus>> {
        Box::pin(route::app_status(self.state.clone()))
    }

    fn toggle_pause(&self) -> BoxFuture<Result<AppStatus>> {
        Box::pin(route::toggle_pause(self.state.clone()))
    }

    fn search_docs(&self, query: SearchParam) -> BoxFuture<Result<SearchResults>> {
        Box::pin(route::search(self.state.clone(), query))
    }

    fn search_lenses(&self, query: SearchLensesParam) -> BoxFuture<Result<SearchLensesResp>> {
        Box::pin(route::search_lenses(self.state.clone(), query))
    }
}

pub fn start_api_ipc(state: &AppState) -> anyhow::Result<Server, ()> {
    let endpoint = gen_ipc_path();

    let mut io = IoHandler::new();
    let rpc = SpyglassRPC {
        state: state.clone(),
    };
    io.extend_with(rpc.to_delegate());

    let server = ServerBuilder::new(io)
        .start(&endpoint)
        .map_err(|_| log::warn!("Couldn't open socket"))
        .unwrap();

    log::info!("Started IPC server at {}", endpoint);
    Ok(server)
}
