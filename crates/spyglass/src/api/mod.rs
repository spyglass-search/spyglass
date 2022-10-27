use jsonrpsee::core::{async_trait, Error};
use libspyglass::state::AppState;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use jsonrpsee::http_server::{HttpServerBuilder, HttpServerHandle};

use shared::request::{SearchLensesParam, SearchParam};
use shared::response as resp;
use spyglass_rpc::RpcServer;

mod auth;
mod response;
mod route;

pub struct SpyglassRpc {
    state: AppState,
}

#[async_trait]
impl RpcServer for SpyglassRpc {
    fn protocol_version(&self) -> Result<String, Error> {
        Ok("version1".into())
    }

    async fn authorize_connection(&self, id: String) -> Result<(), Error> {
        route::authorize_connection(self.state.clone(), id).await
    }

    async fn app_status(&self) -> Result<resp::AppStatus, Error> {
        route::app_status(self.state.clone()).await
    }

    async fn crawl_stats(&self) -> Result<resp::CrawlStats, Error> {
        route::crawl_stats(self.state.clone()).await
    }

    async fn delete_doc(&self, id: String) -> Result<(), Error> {
        route::delete_doc(self.state.clone(), id).await
    }

    async fn delete_domain(&self, domain: String) -> Result<(), Error> {
        route::delete_domain(self.state.clone(), domain).await
    }

    async fn list_connections(&self) -> Result<Vec<resp::ConnectionResult>, Error> {
        route::list_connections(self.state.clone()).await
    }

    async fn list_installed_lenses(&self) -> Result<Vec<resp::LensResult>, Error> {
        route::list_installed_lenses(self.state.clone()).await
    }

    async fn list_plugins(&self) -> Result<Vec<resp::PluginResult>, Error> {
        route::list_plugins(self.state.clone()).await
    }

    async fn recrawl_domain(&self, domain: String) -> Result<(), Error> {
        route::recrawl_domain(self.state.clone(), domain).await
    }

    async fn search_docs(&self, query: SearchParam) -> Result<resp::SearchResults, Error> {
        route::search(self.state.clone(), query).await
    }

    async fn search_lenses(
        &self,
        query: SearchLensesParam,
    ) -> Result<resp::SearchLensesResp, Error> {
        route::search_lenses(self.state.clone(), query).await
    }

    async fn toggle_pause(&self, is_paused: bool) -> Result<(), Error> {
        route::toggle_pause(self.state.clone(), is_paused).await
    }

    async fn toggle_plugin(&self, name: String) -> Result<(), Error> {
        route::toggle_plugin(self.state.clone(), name).await
    }
}

pub async fn start_api_server(state: AppState) -> anyhow::Result<(SocketAddr, HttpServerHandle)> {
    let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), state.user_settings.port);
    let server = HttpServerBuilder::default().build(server_addr).await?;

    let rpc_module = SpyglassRpc {
        state: state.clone(),
    };
    let addr = server.local_addr()?;
    let server_handle = server.start(rpc_module.into_rpc())?;

    log::info!("starting server @ {}", addr);
    Ok((addr, server_handle))
}
