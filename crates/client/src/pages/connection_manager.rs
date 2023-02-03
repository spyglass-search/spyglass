use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use gloo::timers::callback::Interval;
use shared::event::{AuthorizeConnectionParams, ClientEvent, ClientInvoke, ResyncConnectionParams};
use shared::response::{ListConnectionResult, SupportedConnection, UserConnection};

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::{
    btn,
    btn::{BtnSize, BtnType},
    icons, Header,
};
use crate::utils::RequestState;
use crate::{listen, tauri_invoke};

#[derive(Default)]
struct ConnectionStatus {
    is_authorizing: RequestState,
    error: String,
}

#[derive(Default)]
pub struct ConnectionsManagerPage {
    conn_status: ConnectionStatus,
    fetch_connection_state: RequestState,
    fetch_error: String,
    is_add_view: bool,
    supported_connections: Vec<SupportedConnection>,
    supported_map: HashMap<String, SupportedConnection>,
    user_connections: Vec<UserConnection>,
    revoke_requested: HashSet<String>,
    update_interval_handle: Option<Interval>,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum Msg {
    AuthorizeConnection(String),
    // Received an error authorizing connection
    AuthError(String),
    // Authorization finished!
    AuthFinished,
    FetchConnections,
    FetchError(String),
    StartAdd,
    CancelAdd,
    RevokeConnection { id: String, account: String },
    ResyncConnection { id: String, account: String },
    UpdateConnections(ListConnectionResult),
}

impl ConnectionsManagerPage {
    pub async fn fetch_connections() -> Result<ListConnectionResult, String> {
        tauri_invoke(ClientInvoke::ListConnections, "".to_string()).await
    }

    fn add_view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let conns = self
            .supported_connections
            .iter()
            .map(|con| {
                let auth_msg = Msg::AuthorizeConnection(con.id.clone());
                // Annoyingly we need to use Google branded icons for the connection
                // button.
                let connect_btn = if con.id.ends_with("google.com") {
                    html! {
                        <button
                            disabled={self.conn_status.is_authorizing.in_progress()}
                            onclick={link.callback(move |_| auth_msg.clone())}
                        >
                            {
                                if self.conn_status.is_authorizing.in_progress() {
                                    html!{ <icons::GoogleSignInDisabled width="w-auto" height="h-10" /> }
                                } else {
                                    html!{ <icons::GoogleSignIn width="w-auto" height="h-10" /> }
                                }
                            }
                        </button>
                    }
                } else {
                    html! {
                        <btn::Btn
                            disabled={self.conn_status.is_authorizing.in_progress()}
                            onclick={link.callback(move |_| auth_msg.clone())}
                        >
                            <icons::LightningBoltIcon classes="mr-2" width="w-4" height="h-4" />
                            {"Connect"}
                        </btn::Btn>
                    }
                };

                html! {
                    <div class="pb-8 flex flex-row items-center gap-8">
                        <div class="flex-none">
                            {icons::connection_icon(&con.id, "h-6", "w-6", classes!())}
                        </div>
                        <div class="flex-1">
                            <div><h2 class="text-lg">{con.label.clone()}</h2></div>
                            <div class="text-xs text-neutral-400">{con.description.clone()}</div>
                        </div>
                        <div class="flex-none flex flex-col">
                            <div class="ml-auto">{connect_btn}</div>
                        </div>
                    </div>
                }
            })
            .collect::<Html>();

        html! {
            <div>
                <Header label="Connections">
                    <btn::Btn onclick={link.callback(|_| Msg::CancelAdd)}>{"Cancel"}</btn::Btn>
                </Header>
                <div class="px-8 py-4 bg-neutral-800">
                    <div class="mb-4 text-sm">
                    {
                        match self.conn_status.is_authorizing {
                            RequestState::Error => html! {
                                <span class="text-red-400">{self.conn_status.error.clone()}</span>
                            },
                            RequestState::InProgress => html! {
                                <span class="text-cyan-500">
                                    <div>{"Sign-in has opened in a new window. Please authorize to complete connection."}</div>
                                </span>
                            },
                            _ => { html! {} }
                        }
                    }
                    </div>
                    {conns}
                </div>
            </div>
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

        let interval = {
            let link = link.clone();
            Interval::new(10_000, move || link.send_message(Msg::FetchConnections))
        };

        Self {
            update_interval_handle: Some(interval),
            ..Default::default()
        }
    }

    fn destroy(&mut self, _ctx: &Context<Self>) {
        if let Some(handle) = self.update_interval_handle.take() {
            handle.cancel();
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::AuthorizeConnection(id) => {
                self.conn_status.is_authorizing = RequestState::InProgress;

                link.send_future(async move {
                    let id = id.clone();
                    if let Err(err) = tauri_invoke::<_, ()>(
                        ClientInvoke::AuthorizeConnection,
                        &AuthorizeConnectionParams { id: id.clone() },
                    )
                    .await
                    {
                        Msg::AuthError(err)
                    } else {
                        Msg::AuthFinished
                    }
                });

                true
            }
            Msg::AuthError(error) => {
                self.conn_status.is_authorizing = RequestState::Error;
                self.conn_status.error = error;
                true
            }
            Msg::AuthFinished => {
                self.conn_status.is_authorizing = RequestState::Finished;
                self.is_add_view = false;
                link.send_message(Msg::FetchConnections);
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
            Msg::RevokeConnection { id, account } => {
                self.revoke_requested.insert(format!("{account}@{id}"));
                spawn_local(async move {
                    // Revoke & then refresh connections
                    let _ = tauri_invoke::<_, ()>(
                        ClientInvoke::RevokeConnection,
                        &ResyncConnectionParams {
                            id: id.clone(),
                            account: account.clone(),
                        },
                    )
                    .await;
                });

                true
            }
            Msg::ResyncConnection { id, account } => {
                spawn_local(async move {
                    // Revoke & then refresh connections
                    let _ = tauri_invoke::<_, ()>(
                        ClientInvoke::ResyncConnection,
                        &ResyncConnectionParams {
                            id: id.clone(),
                            account: account.clone(),
                        },
                    )
                    .await;
                });
                true
            }
            Msg::UpdateConnections(conns) => {
                self.fetch_connection_state = RequestState::Finished;

                self.supported_connections = conns.supported.clone();
                self.supported_connections
                    .sort_by(|a, b| a.label.cmp(&b.label));

                self.supported_map.clear();
                for conn in &self.supported_connections {
                    self.supported_map.insert(conn.id.clone(), conn.clone());
                }

                self.user_connections = conns.user_connections;
                self.user_connections.sort_by(|a, b| match a.id.cmp(&b.id) {
                    Ordering::Equal => a.account.cmp(&b.account),
                    ord => ord,
                });

                true
            }
            Msg::StartAdd => {
                self.is_add_view = true;
                true
            }
            Msg::CancelAdd => {
                if self.conn_status.is_authorizing.in_progress() {
                    self.conn_status.is_authorizing = RequestState::Error;
                    self.conn_status.error = "Cancelled auth request".to_string();
                }

                self.is_add_view = false;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if self.is_add_view {
            return self.add_view(ctx);
        }

        let link = ctx.link();
        if self.user_connections.is_empty() {
            return self.add_view(ctx);
        }

        let conns = self
            .user_connections
            .iter()
            .map(|conn| {
                let label = self
                    .supported_map
                    .get(&conn.id)
                    .map(|m| m.label.clone())
                    .unwrap_or_else(|| conn.id.clone());

                let resync_msg = Msg::ResyncConnection {
                    id: conn.id.clone(),
                    account: conn.account.clone(),
                };
                let revoke_msg = Msg::RevokeConnection {
                    id: conn.id.clone(),
                    account: conn.account.clone(),
                };

                let uid = format!("{}@{}", conn.account, conn.id);
                html! {
                    <Connection
                        label={label}
                        connection={conn.clone()}
                        is_syncing={conn.is_syncing}
                        is_revoking={self.revoke_requested.contains(&uid)}
                        on_resync={link.callback(move |_| resync_msg.clone())}
                        on_revoke={link.callback(move |_| revoke_msg.clone())}
                    />
                }
            })
            .collect::<Html>();

        html! {
            <div>
                <Header label="Connections">
                    <btn::Btn onclick={link.callback(|_| Msg::StartAdd)}>{"Add"}</btn::Btn>
                </Header>
                <div class="flex flex-col gap-4 p-4">
                    {
                        if self.fetch_connection_state.in_progress() {
                            html! {
                                <div class="flex justify-center">
                                    <div class="p-16">
                                        <icons::RefreshIcon height={"h-16"} width={"w-16"} animate_spin={true} />
                                    </div>
                                </div>
                            }
                        } else {
                            conns
                        }
                    }
                </div>
            </div>
        }
    }
}

#[derive(Properties, PartialEq)]
struct ConnectionProps {
    label: String,
    connection: UserConnection,
    is_syncing: bool,
    is_revoking: bool,
    #[prop_or_default]
    on_resync: Callback<MouseEvent>,
    #[prop_or_default]
    on_revoke: Callback<MouseEvent>,
}

#[function_component(Connection)]
fn connection_comp(props: &ConnectionProps) -> Html {
    let is_resyncing = use_state_eq(|| false);
    is_resyncing.set(props.is_syncing);

    let is_revoking = use_state_eq(|| false);
    is_revoking.set(props.is_revoking);

    let is_resync = is_resyncing.clone();
    let on_resync_cb = props.on_resync.clone();
    let resync_cb = Callback::from(move |e| {
        is_resync.set(true);
        on_resync_cb.emit(e);
    });

    let resync_btn = html! {
        <btn::Btn disabled={*is_resyncing} size={BtnSize::Xs} onclick={resync_cb}>
            <icons::RefreshIcon width="w-4" height="h-4" animate_spin={*is_resyncing} />
            { if *is_resyncing { "Syncing" } else { "Resync"} }
        </btn::Btn>
    };

    let is_revoke = is_revoking.clone();
    let on_revoke_cb = props.on_revoke.clone();
    let revoke_cb = Callback::from(move |e| {
        is_revoke.set(true);
        on_revoke_cb.emit(e);
    });

    let revoke_btn = if *is_revoking {
        html! {
            <btn::Btn disabled={true} size={BtnSize::Xs} _type={BtnType::Danger}>
                {"Deleting"}
            </btn::Btn>
        }
    } else {
        html! {
            <btn::Btn disabled={*is_revoking} size={BtnSize::Xs} _type={BtnType::Danger} onclick={revoke_cb}>
                <icons::TrashIcon width="w-4" height="h-4" />
                {"Delete"}
            </btn::Btn>
        }
    };

    html! {
        <div class="rounded-md bg-neutral-700 p-4 text-white shadow-md flex flex-row gap-4 items-center">
            <div>
                {icons::connection_icon(&props.connection.id, "h-6", "w-6", classes!())}
            </div>
            <div>
                <div class="text-xs font-bold text-cyan-500">{props.label.clone()}</div>
                <div class="text-sm">{props.connection.account.clone()}</div>
            </div>
            <div class="flex flex-row gap-4 grow place-content-end">
                {resync_btn}
                {revoke_btn}
            </div>
        </div>
    }
}
