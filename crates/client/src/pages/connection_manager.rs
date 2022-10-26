use shared::event::AuthorizeConnectionParams;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::{btn, icons, Header};
use crate::utils::RequestState;
use crate::{invoke, listen};
use shared::{
    event::{ClientEvent, ClientInvoke},
    response::ConnectionResult,
};

struct ConnectionStatus {
    is_authorizing: RequestState,
    metadata: ConnectionResult,
}

pub struct ConnectionsManagerPage {
    connections: HashMap<String, ConnectionStatus>,
    fetch_error: String,
    fetch_connection_state: RequestState,
}

#[derive(Clone)]
pub enum Msg {
    AuthorizeConnection(String),
    FetchConnections,
    FetchError(String),
    RevokeConnection,
    UpdateConnections(Vec<ConnectionResult>),
}

impl ConnectionsManagerPage {
    pub async fn fetch_connections() -> Result<Vec<ConnectionResult>, String> {
        match invoke(ClientInvoke::ListConnections.as_ref(), JsValue::NULL).await {
            Ok(results) => match serde_wasm_bindgen::from_value(results) {
                Ok(parsed) => Ok(parsed),
                Err(err) => Err(err.to_string()),
            },
            Err(e) => Err(format!("Error fetching connections: {:?}", e.as_string())),
        }
    }
}

impl Component for ConnectionsManagerPage {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();
        link.send_message(Msg::FetchConnections);

        // Listen to changes in authorized connections
        {
            let link = link.clone();
            spawn_local(async move {
                let cb = Closure::wrap(Box::new(move |_| {
                    link.send_message(Msg::FetchConnections);
                }) as Box<dyn Fn(JsValue)>);

                let _ = listen(ClientEvent::RefreshConnections.as_ref(), &cb).await;
                cb.forget();
            });
        }

        Self {
            connections: HashMap::new(),
            fetch_connection_state: RequestState::NotStarted,
            fetch_error: String::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::AuthorizeConnection(name) => {
                if let Some(status) = self.connections.get_mut(&name) {
                    status.is_authorizing = RequestState::InProgress;
                }

                spawn_local(async move {
                    if let Ok(ser) = serde_wasm_bindgen::to_value(&AuthorizeConnectionParams {
                        name: name.clone(),
                    }) {
                        let _ = invoke(ClientInvoke::AuthorizeConnection.as_ref(), ser).await;
                    }
                });

                true
            }
            Msg::FetchConnections => {
                link.send_future(async {
                    match Self::fetch_connections().await {
                        Ok(conns) => Msg::UpdateConnections(conns),
                        Err(err) => Msg::FetchError(err),
                    }
                });
                false
            }
            Msg::FetchError(error) => {
                self.fetch_connection_state = RequestState::Error;
                self.fetch_error = error;
                true
            }
            Msg::RevokeConnection => false,
            Msg::UpdateConnections(conns) => {
                self.fetch_connection_state = RequestState::Finished;
                self.connections = conns
                    .iter()
                    .map(|conn| {
                        (
                            conn.name.clone(),
                            ConnectionStatus {
                                is_authorizing: RequestState::NotStarted,
                                metadata: conn.clone(),
                            },
                        )
                    })
                    .collect::<HashMap<String, ConnectionStatus>>();

                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let conns = self.connections.values()
            .map(|status| {
                let auth_msg = Msg::AuthorizeConnection(status.metadata.name.clone());
                html! {
                    <div class="pb-8 flex flex-row items-center">
                        <div class="flex-1">
                            <div><h2 class="text-lg">{status.metadata.name.clone()}</h2></div>
                            <div class="text-xs truncate text-neutral-400">{"Description of the integration"}</div>
                        </div>
                        <div class="flex-none">
                            {
                                if status.metadata.is_connected {
                                    html! {
                                        <btn::Btn onclick={link.callback(|_| Msg::RevokeConnection)}>
                                            <icons::XCircle classes="mr-2" width="w-4" height="h-4" />
                                            {"Revoke"}
                                        </btn::Btn>
                                    }
                                } else {
                                    html! {
                                        <btn::Btn
                                            disabled={status.is_authorizing.in_progress()}
                                            onclick={link.callback(move |_| auth_msg.clone())}
                                        >
                                            {
                                                if status.is_authorizing.in_progress() {
                                                    html! {
                                                        <icons::RefreshIcon animate_spin={true} classes="mr-2" width="w-4" height="h-4" />
                                                    }
                                                } else {
                                                    html! {
                                                        <icons::LightningBoltIcon classes="mr-2" width="w-4" height="h-4" />
                                                    }
                                                }
                                            }
                                            {"Connect"}
                                        </btn::Btn>
                                    }
                                }
                            }
                        </div>
                    </div>
                }
            })
            .collect::<Html>();

        html! {
            <div>
                <Header label="Connections">
                    <button>
                        {"Save & Restart"}
                    </button>
                </Header>
                <div class="p-8 bg-neutral-800">
                    {conns}
                </div>
            </div>
        }
    }
}
