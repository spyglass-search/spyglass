use entities::get_library_stats;
use entities::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use jsonrpsee::core::{async_trait, Error};
use libspyglass::search;
use libspyglass::state::AppState;
use libspyglass::task::{CollectTask, ManagerCommand};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use jsonrpsee::server::{ServerBuilder, ServerHandle};

use shared::config::Config;
use shared::request::{SearchLensesParam, SearchParam};
use shared::response::{self as resp, LibraryStats};
use spyglass_rpc::RpcServer;

mod auth;
mod response;
mod route;

pub struct SpyglassRpc {
    state: AppState,
    config: Config,
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

    async fn delete_doc(&self, id: String) -> Result<(), Error> {
        route::delete_doc(self.state.clone(), id).await
    }

    async fn delete_domain(&self, domain: String) -> Result<(), Error> {
        route::delete_domain(self.state.clone(), domain).await
    }

    async fn get_library_stats(&self) -> Result<HashMap<String, LibraryStats>, Error> {
        match get_library_stats(self.state.db.clone()).await {
            Ok(stats) => Ok(stats),
            Err(err) => {
                log::error!("Unable to get library stats: {}", err);
                Ok(HashMap::new())
            }
        }
    }

    async fn list_connections(&self) -> Result<resp::ListConnectionResult, Error> {
        route::list_connections(self.state.clone()).await
    }

    async fn list_installed_lenses(&self) -> Result<Vec<resp::LensResult>, Error> {
        route::list_installed_lenses(self.state.clone()).await
    }

    async fn install_lens(&self, lens_name: String) -> Result<(), Error> {
        if let Err(error) = search::lens::install_lens(&self.state, &self.config, lens_name).await {
            return Err(Error::Custom(error.to_string()));
        }
        Ok(())
    }

    async fn list_plugins(&self) -> Result<Vec<resp::PluginResult>, Error> {
        route::list_plugins(self.state.clone()).await
    }

    async fn recrawl_domain(&self, domain: String) -> Result<(), Error> {
        route::recrawl_domain(self.state.clone(), domain).await
    }

    async fn resync_connection(&self, api_id: String, account: String) -> Result<(), Error> {
        let _ = self
            .state
            .schedule_work(ManagerCommand::Collect(CollectTask::ConnectionSync {
                api_id,
                account,
            }))
            .await;

        Ok(())
    }

    /// Remove connection from list of connections
    async fn revoke_connection(&self, api_id: String, account: String) -> Result<(), Error> {
        use entities::models::connection;
        // Remove from connections list
        let _ = connection::Entity::delete_many()
            .filter(connection::Column::ApiId.eq(api_id.clone()))
            .filter(connection::Column::Account.eq(account))
            .exec(&self.state.db)
            .await;

        // Remove from index
        let _ = self.delete_domain(api_id).await;
        Ok(())
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

    async fn uninstall_lens(&self, name: String) -> Result<(), Error> {
        route::uninstall_lens(self.state.clone(), &self.config, &name).await
    }
}

pub async fn start_api_server(
    state: AppState,
    config: Config,
) -> anyhow::Result<(SocketAddr, ServerHandle)> {
    let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), state.user_settings.port);
    let server = ServerBuilder::default().build(server_addr).await?;

    let rpc_module = SpyglassRpc {
        state,
        config: config.clone(),
    };
    let addr = server.local_addr()?;
    let server_handle = server.start(rpc_module.into_rpc())?;

    log::info!("starting server @ {}", addr);
    Ok((addr, server_handle))
}
