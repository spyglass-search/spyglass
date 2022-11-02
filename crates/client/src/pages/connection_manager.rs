use std::collections::{HashMap, HashSet};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::{btn, icons, Header};
use crate::utils::RequestState;
use crate::{invoke, listen};
use shared::{
    event::{AuthorizeConnectionParams, ClientEvent, ClientInvoke},
    response::ConnectionResult,
};

struct ConnectionStatus {
    is_authorizing: RequestState,
    error: String,
}

pub struct ConnectionsManagerPage {
    connections: Vec<ConnectionResult>,
    conn_status: HashMap<String, ConnectionStatus>,
    fetch_error: String,
    fetch_connection_state: RequestState,
    resync_requested: HashSet<String>,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum Msg {
    AuthorizeConnection(String),
    // Received an error authorizing connection
    AuthError(String, String),
    // Authorization finished!
    AuthFinished(String),
    FetchConnections,
    FetchError(String),
    RevokeConnection(String),
    ResyncConnection(String),
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

    pub fn connection_icon(&self, id: &str) -> Html {
        if id == "calendar.google.com" {
            html! {
                <svg class="h-6 w-6" role="img" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path d="M18.316 5.684H24v12.632h-5.684V5.684zM5.684 24h12.632v-5.684H5.684V24zM18.316 5.684V0H1.895A1.894 1.894 0 0 0 0 1.895v16.421h5.684V5.684h12.632zm-7.207 6.25v-.065c.272-.144.5-.349.687-.617s.279-.595.279-.982c0-.379-.099-.72-.3-1.025a2.05 2.05 0 0 0-.832-.714 2.703 2.703 0 0 0-1.197-.257c-.6 0-1.094.156-1.481.467-.386.311-.65.671-.793 1.078l1.085.452c.086-.249.224-.461.413-.633.189-.172.445-.257.767-.257.33 0 .602.088.816.264a.86.86 0 0 1 .322.703c0 .33-.12.589-.36.778-.24.19-.535.284-.886.284h-.567v1.085h.633c.407 0 .748.109 1.02.327.272.218.407.499.407.843 0 .336-.129.614-.387.832s-.565.327-.924.327c-.351 0-.651-.103-.897-.311-.248-.208-.422-.502-.521-.881l-1.096.452c.178.616.505 1.082.977 1.401.472.319.984.478 1.538.477a2.84 2.84 0 0 0 1.293-.291c.382-.193.684-.458.902-.794.218-.336.327-.72.327-1.149 0-.429-.115-.797-.344-1.105a2.067 2.067 0 0 0-.881-.689zm2.093-1.931l.602.913L15 10.045v5.744h1.187V8.446h-.827l-2.158 1.557zM22.105 0h-3.289v5.184H24V1.895A1.894 1.894 0 0 0 22.105 0zm-3.289 23.5l4.684-4.684h-4.684V23.5zM0 22.105C0 23.152.848 24 1.895 24h3.289v-5.184H0v3.289z"/>
                </svg>
            }
        } else if id == "drive.google.com" {
            html! {
                <svg class="h-6 w-6" role="img" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path d="M12.01 1.485c-2.082 0-3.754.02-3.743.047.01.02 1.708 3.001 3.774 6.62l3.76 6.574h3.76c2.081 0 3.753-.02 3.742-.047-.005-.02-1.708-3.001-3.775-6.62l-3.76-6.574zm-4.76 1.73a789.828 789.861 0 0 0-3.63 6.319L0 15.868l1.89 3.298 1.885 3.297 3.62-6.335 3.618-6.33-1.88-3.287C8.1 4.704 7.255 3.22 7.25 3.214zm2.259 12.653-.203.348c-.114.198-.96 1.672-1.88 3.287a423.93 423.948 0 0 1-1.698 2.97c-.01.026 3.24.042 7.222.042h7.244l1.796-3.157c.992-1.734 1.85-3.23 1.906-3.323l.104-.167h-7.249z"/>
                </svg>
            }
        } else if id == "mail.google.com" {
            html! {
                <svg class="h-6 w-6" role="img" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path d="M24 5.457v13.909c0 .904-.732 1.636-1.636 1.636h-3.819V11.73L12 16.64l-6.545-4.91v9.273H1.636A1.636 1.636 0 0 1 0 19.366V5.457c0-2.023 2.309-3.178 3.927-1.964L5.455 4.64 12 9.548l6.545-4.91 1.528-1.145C21.69 2.28 24 3.434 24 5.457z"/>
                </svg>
            }
        } else {
            html! { <icons::ShareIcon height="h-6" width="w-6" /> }
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
            connections: Vec::new(),
            conn_status: HashMap::new(),
            fetch_connection_state: RequestState::NotStarted,
            fetch_error: String::new(),
            resync_requested: HashSet::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::AuthorizeConnection(id) => {
                if let Some(status) = self.conn_status.get_mut(&id) {
                    status.is_authorizing = RequestState::InProgress;
                }

                link.send_future(async move {
                    let id = id.clone();

                    let ser =
                        serde_wasm_bindgen::to_value(&AuthorizeConnectionParams { id: id.clone() })
                            .expect("Unable to serialize authorize connection params");

                    if let Err(err) = invoke(ClientInvoke::AuthorizeConnection.as_ref(), ser).await
                    {
                        let msg = err
                            .as_string()
                            .unwrap_or_else(|| "Unable to connect. Please try again.".to_string());
                        Msg::AuthError(id.clone(), msg)
                    } else {
                        Msg::AuthFinished(id.clone())
                    }
                });

                true
            }
            Msg::AuthError(id, error) => {
                if let Some(status) = self.conn_status.get_mut(&id) {
                    status.is_authorizing = RequestState::Error;
                    status.error = error;
                }
                true
            }
            Msg::AuthFinished(id) => {
                if let Some(status) = self.conn_status.get_mut(&id) {
                    status.is_authorizing = RequestState::Finished;
                    link.send_message(Msg::FetchConnections);
                }
                false
            }
            Msg::FetchConnections => {
                if self.fetch_connection_state.in_progress() {
                    return false;
                }

                self.fetch_connection_state = RequestState::InProgress;
                link.send_future(async {
                    match Self::fetch_connections().await {
                        Ok(conns) => Msg::UpdateConnections(conns),
                        Err(err) => Msg::FetchError(err),
                    }
                });
                false
            }
            Msg::FetchError(error) => {
                log::error!("Error fetching: {}", error);
                self.fetch_connection_state = RequestState::Error;
                self.fetch_error = error;
                true
            }
            Msg::RevokeConnection(id) => {
                let ser = serde_wasm_bindgen::to_value(&AuthorizeConnectionParams { id })
                    .expect("Unable to serialize authorize connection params");

                link.send_future(async {
                    // Revoke & then refresh connections
                    let _ = invoke(ClientInvoke::RevokeConnection.as_ref(), ser).await;
                    Msg::FetchConnections
                });

                true
            }
            Msg::ResyncConnection(id) => {
                let ser =
                    serde_wasm_bindgen::to_value(&AuthorizeConnectionParams { id: id.clone() })
                        .expect("Unable to serialize authorize connection params");

                link.send_future(async {
                    // Revoke & then refresh connections
                    let _ = invoke(ClientInvoke::ResyncConnection.as_ref(), ser).await;
                    Msg::FetchConnections
                });

                self.resync_requested.insert(id);
                true
            }
            Msg::UpdateConnections(conns) => {
                self.fetch_connection_state = RequestState::Finished;
                self.connections = conns.clone();
                self.conn_status = conns
                    .iter()
                    .map(|conn| {
                        (
                            conn.id.clone(),
                            ConnectionStatus {
                                is_authorizing: RequestState::NotStarted,
                                error: String::new(),
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
        let conns = self.connections.iter()
            .map(|con| {
                let status = self.conn_status.get(&con.id).expect("Unknown connection");
                let auth_msg = Msg::AuthorizeConnection(con.id.clone());
                // let revoke_msg = Msg::RevokeConnection(status.metadata.id.clone());
                let resync_msg = Msg::ResyncConnection(con.id.clone());
                let resynced = self.resync_requested.contains(&con.id.clone());

                let connect_btn = if con.is_connected {
                    html! {
                        <div class="flex flex-row gap-4">
                            <btn::Btn onclick={link.callback(move |_| resync_msg.clone())} disabled={resynced}>
                                <icons::RefreshIcon classes="mr-2" width="w-4" height="h-4" />
                                {"Resync"}
                            </btn::Btn>
                            // <btn::Btn onclick={link.callback(move |_| revoke_msg.clone())}>
                            //     <icons::XCircle classes="mr-2" width="w-4" height="h-4" />
                            //     {"Revoke"}
                            // </btn::Btn>
                        </div>
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
                                        <>
                                            <icons::RefreshIcon animate_spin={true} classes="mr-2" width="w-4" height="h-4" />
                                            {"Connecting"}
                                        </>
                                    }
                                } else {
                                    html! {
                                        <>
                                            <icons::LightningBoltIcon classes="mr-2" width="w-4" height="h-4" />
                                            {"Connect"}
                                        </>
                                    }
                                }
                            }
                        </btn::Btn>
                    }
                };

                html! {
                    <div class="pb-8 flex flex-row items-center gap-8">
                        <div class="flex-none">
                            {self.connection_icon(&con.id)}
                        </div>
                        <div class="flex-1">
                            <div><h2 class="text-lg">{con.label.clone()}</h2></div>
                            <div class="text-xs text-neutral-400">{con.description.clone()}</div>
                            <div class="text-xs text-red-400">{status.error.clone()}</div>
                        </div>
                        <div class="flex-none">{connect_btn}</div>
                    </div>
                }
            })
            .collect::<Html>();

        html! {
            <div>
                <Header label="Connections">
                </Header>
                <div class="p-8 bg-neutral-800">
                    {conns}
                </div>
            </div>
        }
    }
}
