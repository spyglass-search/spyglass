use std::str::FromStr;

use shared::{keyboard::KeyCode, response::SearchResult};
use ui_components::{
    btn::{Btn, BtnType},
    icons::RefreshIcon,
    results::SearchResultItem,
};
use web_sys::HtmlInputElement;
use yew::{platform::spawn_local, prelude::*};

use crate::constants::SEARCH_ENDPOINT;
const RESULT_PREFIX: &str = "result";

#[derive(Clone, Debug)]
pub enum Msg {
    HandleKeyboardEvent(KeyboardEvent),
    HandleSearch,
    SetSearchResults(Vec<SearchResult>),
    OpenResult(SearchResult),
}

pub struct SearchPage {
    client: reqwest::Client,
    results: Vec<SearchResult>,
    search_wrapper_ref: NodeRef,
    search_input_ref: NodeRef,
    status_msg: Option<String>,
    in_progress: bool,
}

impl Component for SearchPage {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &yew::Context<Self>) -> Self {
        Self {
            client: reqwest::Client::new(),
            results: Vec::new(),
            search_input_ref: Default::default(),
            search_wrapper_ref: Default::default(),
            status_msg: None,
            in_progress: false
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

                let query = self
                    .search_input_ref
                    .cast::<HtmlInputElement>()
                    .map(|x| x.value());

                log::info!("handling search! {:?}", query);
                if let Some(query) = query {
                    self.status_msg = Some(format!("searching: {query}"));
                    let client = self.client.clone();
                    let link = link.clone();
                    spawn_local(async move {
                        match client
                            .get(SEARCH_ENDPOINT)
                            .query(&[("query", query)])
                            .send()
                            .await
                        {
                            Ok(result) => match result.json::<Vec<SearchResult>>().await {
                                Ok(res) => link.send_message(Msg::SetSearchResults(res)),
                                Err(err) => {
                                    log::error!("err: {}", err);
                                }
                            },
                            Err(err) => {
                                log::error!("err: {}", err);
                            }
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
                <div class="flex flex-nowrap w-full bg-neutral-800 p-4">
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
                <div class="w-full flex flex-col">{results}</div>
                <div class="border-t-2 border-neutral-900 p-4">
                    {self.status_msg.clone().unwrap_or_else(|| "how to guide?".into())}
               </div>
            </div>
        }
    }
}
