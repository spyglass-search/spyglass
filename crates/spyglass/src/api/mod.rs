extern crate jsonrpc_ipc_server;

use jsonrpc_core::{BoxFuture, IoHandler, Result};
use jsonrpc_ipc_server::{Server, ServerBuilder};

use libspyglass::state::AppState;

use shared::request::{SearchLensesParam, SearchParam};
use shared::response::{AppStatus, CrawlStats, LensResult, SearchLensesResp, SearchResults};
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

    fn crawl_stats(&self) -> BoxFuture<Result<CrawlStats>> {
        Box::pin(route::crawl_stats(self.state.clone()))
    }

    fn delete_doc(&self, id: String) -> BoxFuture<Result<()>> {
        Box::pin(route::delete_doc(self.state.clone(), id))
    }

    fn list_installed_lenses(&self) -> BoxFuture<Result<Vec<LensResult>>> {
        Box::pin(route::list_installed_lenses(self.state.clone()))
    }

    fn recrawl_domain(&self, domain: String) -> BoxFuture<Result<()>> {
        Box::pin(route::recrawl_domain(self.state.clone(), domain))
    }

    fn search_docs(&self, query: SearchParam) -> BoxFuture<Result<SearchResults>> {
        Box::pin(route::search(self.state.clone(), query))
    }

    fn search_lenses(&self, query: SearchLensesParam) -> BoxFuture<Result<SearchLensesResp>> {
        Box::pin(route::search_lenses(self.state.clone(), query))
    }

    fn toggle_pause(&self) -> BoxFuture<Result<AppStatus>> {
        Box::pin(route::toggle_pause(self.state.clone()))
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
