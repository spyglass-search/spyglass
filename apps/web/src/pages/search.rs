use crate::client::SpyglassClient;
use futures::lock::Mutex;
use shared::keyboard::KeyCode;
use shared::response::SearchResult;
use std::str::FromStr;
use std::sync::Arc;
use ui_components::{
    btn::{Btn, BtnType},
    icons::RefreshIcon,
    results::SearchResultItem,
};
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlInputElement};
use yew::prelude::*;

// make sure we only have one connection per client
type Client = Arc<Mutex<SpyglassClient>>;

const RESULT_PREFIX: &str = "result";

#[derive(Clone, Debug)]
pub enum Msg {
    HandleKeyboardEvent(KeyboardEvent),
    HandleSearch,
    SetSearchResults(Vec<SearchResult>),
    SetError(String),
    OpenResult(SearchResult),
}

pub struct SearchPage {
    client: Client,
    results: Vec<SearchResult>,
    search_wrapper_ref: NodeRef,
    search_input_ref: NodeRef,
    status_msg: Option<String>,
    in_progress: bool,
}

impl Component for SearchPage {
    type Message = Msg;
    type Properties = ();

    fn create(_: &yew::Context<Self>) -> Self {
        Self {
            client: Arc::new(Mutex::new(SpyglassClient::new())),
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
                    let client = self.client.clone();
                    spawn_local(async move {
                        let mut client = client.lock().await;
                        if let Err(err) = client.search(&query, link.clone()).await {
                            log::error!("{}", err.to_string());
                            link.send_message(Msg::SetError(err.to_string()));
                        }
                    });
                }
                true
            }
            Msg::SetSearchResults(results) => {
                self.in_progress = false;
                self.results = results;
                true
            }
            Msg::SetError(err) => {
                self.in_progress = false;
                self.status_msg = Some(err);
                true
            }
            Msg::OpenResult(result) => {
                log::info!("opening result: {}", result.url);
                if let Some(window) = window() {
                    let _ = window.open_with_url_and_target(&result.url, "blank");
                }

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
