use jsonrpsee_core::{client::ClientT, rpc_params};
use jsonrpsee_wasm_client::{Client, WasmClientBuilder};
use shared::request::SearchParam;
use shared::{
    keyboard::KeyCode,
    response::{SearchResult, SearchResults},
};
use std::{
    str::FromStr,
    sync::{Arc, Mutex},
};
use ui_components::{
    btn::{Btn, BtnType},
    icons::RefreshIcon,
    results::SearchResultItem,
};
use web_sys::HtmlInputElement;
use yew::{platform::spawn_local, prelude::*};

use crate::constants::RPC_ENDPOINT;
const RESULT_PREFIX: &str = "result";

pub type RpcMutex = Arc<Mutex<Client>>;

#[derive(Clone, Debug)]
pub enum Msg {
    HandleKeyboardEvent(KeyboardEvent),
    HandleSearch,
    SetClient(RpcMutex),
    SetSearchResults(Vec<SearchResult>),
    OpenResult(SearchResult),
}

pub struct SearchPage {
    rpc_client: Option<RpcMutex>,
    results: Vec<SearchResult>,
    search_wrapper_ref: NodeRef,
    search_input_ref: NodeRef,
    status_msg: Option<String>,
    in_progress: bool,
}

impl Component for SearchPage {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &yew::Context<Self>) -> Self {
        let link = ctx.link();
        link.send_future(async move {
            let client = WasmClientBuilder::default()
                .request_timeout(std::time::Duration::from_secs(10))
                .build(RPC_ENDPOINT)
                .await
                .expect("Unable to create WsClient");
            Msg::SetClient(Arc::new(Mutex::new(client)))
        });

        Self {
            rpc_client: None,
            results: Vec::new(),
            search_input_ref: Default::default(),
            search_wrapper_ref: Default::default(),
            status_msg: None,
            in_progress: false,
        }
    }

    fn update(&mut self, ctx: &yew::Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::SetClient(client) => {
                self.rpc_client = Some(client);
                false
            }
            Msg::HandleKeyboardEvent(event) => {
                let key = event.key();
                if let Ok(code) = KeyCode::from_str(&key.to_uppercase()) {
                    if code == KeyCode::Enter {
                        log::info!("key-code: {code}");
                        link.send_message(Msg::HandleSearch);
                    }
                }
                false
            }
            Msg::HandleSearch => {
                self.in_progress = true;
                self.results = Vec::new();

                let query = self
                    .search_input_ref
                    .cast::<HtmlInputElement>()
                    .map(|x| x.value());

                log::info!("handling search! {:?}", query);
                if let Some(query) = query {
                    self.status_msg = Some(format!("searching: {query}"));
                    let link = link.clone();
                    if let Some(client) = &self.rpc_client {
                        let client = client.clone();
                        spawn_local(async move {
                            if let Ok(client) = client.lock() {
                                let params = SearchParam {
                                    lenses: Vec::new(),
                                    query: query,
                                };
                                match client
                                    .request::<SearchResults, _>(
                                        "spyglass_search_docs",
                                        rpc_params![params],
                                    )
                                    .await
                                {
                                    Ok(res) => {
                                        link.send_message(Msg::SetSearchResults(res.results));
                                    }
                                    Err(err) => {
                                        log::error!("error rpc: {}", err);
                                    }
                                }
                            }
                        });
                    }
                }
                true
            }
            Msg::SetSearchResults(results) => {
                self.in_progress = false;
                self.results = results;
                true
            }
            Msg::OpenResult(result) => {
                log::info!("opening result: {}", result.url);
                false
            }
        }
    }

    fn view(&self, ctx: &yew::Context<Self>) -> yew::Html {
        let link = ctx.link();
        let html = self
            .results
            .iter()
            .enumerate()
            .map(|(idx, res)| {
                let open_msg = Msg::OpenResult(res.to_owned());
                html! {
                    <SearchResultItem
                        id={format!("{RESULT_PREFIX}{idx}")}
                        onclick={link.callback(move |_| open_msg.clone())}
                        result={res.clone()}
                    />
                }
            })
            .collect::<Html>();
        let results = html! { <div class="pb-2">{html}</div> };

        html! {
            <div ref={self.search_wrapper_ref.clone()} class="relative">
                <div class="flex flex-nowrap w-full bg-neutral-800 p-4 border-b-2 border-neutral-900">
                    <input
                        ref={self.search_input_ref.clone()}
                        id="searchbox"
                        type="text"
                        class="bg-neutral-800 text-white text-2xl py-3 overflow-hidden flex-1 outline-none active:outline-none focus:outline-none caret-white placeholder-neutral-600"
                        placeholder="how do i resize a window in tauri?"
                        spellcheck="false"
                        tabindex="-1"
                        onkeyup={link.callback(Msg::HandleKeyboardEvent)}
                    />
                    <Btn
                        _type={BtnType::Primary}
                        onclick={link.callback(|_| Msg::HandleSearch)}
                        disabled={self.in_progress}
                    >
                        {if self.in_progress {
                            html! { <RefreshIcon animate_spin={true} height="h-5" width="w-5" classes={"text-white"} /> }
                        } else {
                            html! { <>{"Search"}</> }
                        }}
                    </Btn>
                </div>
                <div class="w-full flex flex-col animate-fade-in">{results}</div>
            </div>
        }
    }
}
