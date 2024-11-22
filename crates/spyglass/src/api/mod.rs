use crate::task::lens::install_lens;
use entities::get_library_stats;
use entities::models::indexed_document;
use entities::sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter};
use jsonrpsee::core::{async_trait, JsonValue, RpcResult, StringError, SubscriptionResult};
use jsonrpsee::server::middleware::http::ProxyGetRequestLayer;
use jsonrpsee::server::{RpcServiceBuilder, Server, ServerHandle};
use jsonrpsee::{PendingSubscriptionSink, SubscriptionMessage};
use libspyglass::state::AppState;
use libspyglass::task::{CollectTask, ManagerCommand};
use shared::config::{Config, UserSettings};
use shared::llm::{ChatMessage, LlmSession};
use shared::request::{BatchDocumentRequest, RawDocumentRequest, SearchLensesParam, SearchParam};
use shared::response::{self as resp, DefaultIndices, LibraryStats};
use spyglass_rpc::{server_error, RpcEventType, RpcServer};
use spyglass_searcher::WriteTrait;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

mod handler;
mod response;

pub struct SpyglassRpc {
    state: AppState,
    config: Config,
}

#[async_trait]
impl RpcServer for SpyglassRpc {
    fn protocol_version(&self) -> RpcResult<String> {
        Ok("0.1.2".into())
    }

    fn system_health(&self) -> RpcResult<JsonValue> {
        Ok(serde_json::json!({ "health": true }))
    }

    async fn add_raw_document(&self, req: RawDocumentRequest) -> RpcResult<()> {
        handler::add_raw_document(&self.state, &req).await
    }

    async fn add_document_batch(&self, req: BatchDocumentRequest) -> RpcResult<()> {
        handler::add_document_batch(&self.state, &req).await
    }

    async fn authorize_connection(&self, id: String) -> RpcResult<()> {
        handler::authorize_connection(self.state.clone(), id).await
    }

    async fn app_status(&self) -> RpcResult<resp::AppStatus> {
        handler::app_status(self.state.clone()).await
    }

    async fn chat_completion(&self, session: LlmSession) -> RpcResult<ChatMessage> {
        handler::chat_completion(self.state.clone(), &session).await
    }

    /// Default folders used in the local file indexer
    async fn default_indices(&self) -> RpcResult<DefaultIndices> {
        Ok(handler::default_indices().await)
    }

    /// Delete a single doc
    async fn delete_document(&self, id: String) -> RpcResult<()> {
        handler::delete_document(self.state.clone(), id).await
    }

    async fn delete_document_by_url(&self, url: String) -> RpcResult<()> {
        if let Ok(Some(doc)) = indexed_document::Entity::find()
            .filter(indexed_document::Column::Url.eq(url))
            .one(&self.state.db)
            .await
        {
            handler::delete_document(self.state.clone(), doc.doc_id).await
        } else {
            Ok(())
        }
    }

    async fn get_library_stats(&self) -> RpcResult<HashMap<String, LibraryStats>> {
        match get_library_stats(&self.state.db).await {
            Ok(stats) => Ok(stats),
            Err(err) => {
                log::error!("Unable to get library stats: {}", err);
                Ok(HashMap::new())
            }
        }
    }

    async fn is_document_indexed(&self, url: String) -> RpcResult<bool> {
        // Normalize URL
        if let Ok(mut url) = url::Url::parse(&url) {
            url.set_fragment(None);
            let url_str = url.to_string();
            let result = indexed_document::Entity::find()
                .filter(
                    Condition::any()
                        // checks against raw urls that have been added
                        .add(indexed_document::Column::Url.eq(url_str.clone()))
                        // checks against URLs gathered through integrations,
                        // e.g. A starred github repo should match against a github URL
                        // if we have it.
                        .add(indexed_document::Column::OpenUrl.eq(url_str)),
                )
                .one(&self.state.db)
                .await;

            match result {
                Ok(result) => Ok(result.is_some()),
                Err(err) => Err(server_error(format!("Unable to query db: {err}"), None)),
            }
        } else {
            Ok(false)
        }
    }

    async fn list_connections(&self) -> RpcResult<resp::ListConnectionResult> {
        handler::list_connections(self.state.clone()).await
    }

    async fn list_installed_lenses(&self) -> RpcResult<Vec<resp::LensResult>> {
        handler::list_installed_lenses(self.state.clone()).await
    }

    async fn install_lens(&self, lens_name: String) -> RpcResult<()> {
        if let Err(error) = install_lens(&self.state, &self.config, lens_name).await {
            return Err(server_error(error.to_string(), None));
        }
        Ok(())
    }

    async fn list_plugins(&self) -> RpcResult<Vec<resp::PluginResult>> {
        handler::list_plugins(self.state.clone()).await
    }

    async fn recrawl_domain(&self, domain: String) -> RpcResult<()> {
        handler::recrawl_domain(self.state.clone(), domain).await
    }

    async fn resync_connection(&self, api_id: String, account: String) -> RpcResult<()> {
        let _ = self
            .state
            .schedule_work(ManagerCommand::Collect(CollectTask::ConnectionSync {
                api_id,
                account,
                is_first_sync: false,
            }))
            .await;

        Ok(())
    }

    /// Remove connection from list of connections
    async fn revoke_connection(&self, api_id: String, account: String) -> RpcResult<()> {
        use entities::models::connection;
        let url_like = format!("api://{account}@{api_id}%");
        log::debug!("revoking conn: {url_like}");

        // Delete from search index
        let docs = indexed_document::Entity::find()
            .filter(indexed_document::Column::Domain.eq(api_id.clone()))
            .filter(indexed_document::Column::Url.like(&url_like))
            .all(&self.state.db)
            .await
            .unwrap_or_default();

        let doc_ids = docs
            .iter()
            .map(|m| m.doc_id.clone())
            .collect::<Vec<String>>();
        let _ = connection::revoke_connection(&self.state.db, &api_id, &account).await;
        let _ = self.state.index.delete_many_by_id(&doc_ids).await;
        let _ = indexed_document::delete_many_by_doc_id(&self.state.db, &doc_ids).await;
        log::debug!("revoked & deleted {} docs", doc_ids.len());
        Ok(())
    }

    async fn search_docs(&self, query: SearchParam) -> RpcResult<resp::SearchResults> {
        handler::search::search_docs(self.state.clone(), query).await
    }

    async fn search_lenses(&self, query: SearchLensesParam) -> RpcResult<resp::SearchLensesResp> {
        handler::search::search_lenses(self.state.clone(), query).await
    }

    async fn toggle_pause(&self, is_paused: bool) -> RpcResult<()> {
        handler::toggle_pause(self.state.clone(), is_paused).await
    }

    async fn uninstall_lens(&self, name: String) -> RpcResult<()> {
        handler::uninstall_lens(self.state.clone(), &self.config, &name).await
    }

    async fn update_user_settings(&self, settings: UserSettings) -> RpcResult<UserSettings> {
        handler::update_user_settings(&self.state, &self.config, &settings).await
    }

    async fn user_settings(&self) -> RpcResult<UserSettings> {
        handler::user_settings(&self.state).await
    }

    async fn subscribe_events(
        &self,
        sink: PendingSubscriptionSink,
        events: Vec<RpcEventType>,
    ) -> SubscriptionResult {
        let sink = match sink.accept().await {
            Ok(sink) => sink,
            Err(err) => {
                log::warn!("Unable to accept subscription: {err}");
                return Err(StringError::from("SubscriptionEmptyError"));
            }
        };

        // Spawn a new task that listens for events in the channel and sends them out
        let rpc_event_channel = self.state.rpc_events.clone();
        let shutdown_cmd_tx = self.state.shutdown_cmd_tx.clone();
        let mut receiver = rpc_event_channel
            .lock()
            .expect("rpc_events held by another thread")
            .subscribe();
        let mut shutdown = shutdown_cmd_tx.lock().await.subscribe();

        let events: HashSet<RpcEventType> = events.clone().into_iter().collect();
        log::debug!("SUBSCRIBED TO: {:?}", events);
        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    break;
                }
                res = receiver.recv() => {
                    match res {
                        Ok(event) => {
                            if events.contains(&event.event_type) {
                                if let Ok(msg) = SubscriptionMessage::from_json(&event) {
                                    if let Err(err) = sink.send(msg).await {
                                        log::warn!("unable to send to sub: {err}");
                                    }
                                } else {
                                    log::warn!("unable to serialize: {event:?}");
                                }
                            }
                        },
                        Err(err) => {
                            log::warn!("error recv: {err:?}");
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

pub async fn start_api_server(
    addr: Option<IpAddr>,
    state: AppState,
    config: Config,
) -> anyhow::Result<(SocketAddr, ServerHandle)> {
    let middleware = tower::ServiceBuilder::new().layer(
        ProxyGetRequestLayer::new("/health", "spyglass_system_health")
            .expect("Unable to create middleware"),
    );

    let ip = addr.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let server_addr = SocketAddr::new(ip, state.user_settings.load_full().port);

    let rpc_middleware = RpcServiceBuilder::new().rpc_logger(1024);
    let server = Server::builder()
        .set_rpc_middleware(rpc_middleware)
        .set_http_middleware(middleware)
        .build(server_addr)
        .await?;

    let rpc_module = SpyglassRpc {
        state: state.clone(),
        config: config.clone(),
    };

    let addr = server.local_addr()?;
    let server_handle = server.start(rpc_module.into_rpc());
    log::info!("starting server @ {}", addr);
    Ok((addr, server_handle))
}
