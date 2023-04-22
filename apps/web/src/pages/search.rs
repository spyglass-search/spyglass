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
    SetStatus(String),
    TokenReceived(String),
    SetFinished,
    OpenResult(SearchResult),
}

pub struct SearchPage {
    client: Client,
    results: Vec<SearchResult>,
    search_wrapper_ref: NodeRef,
    search_input_ref: NodeRef,
    status_msg: Option<String>,
    tokens: Option<String>,
    current_query: Option<String>,
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
            tokens: None,
            current_query: None,
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
                self.tokens = None;
                self.status_msg = None;
                self.results = Vec::new();

                if let Some(search_input) = self.search_input_ref.cast::<HtmlInputElement>() {
                    let query = search_input.value();
                    log::info!("handling search! {:?}", query);

                    self.current_query = Some(query.clone());
                    search_input.set_value("");
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
                self.results = results;
                true
            }
            Msg::SetError(err) => {
                self.in_progress = false;
                self.status_msg = Some(err);
                true
            }
            Msg::SetFinished => {
                self.in_progress = false;
                self.status_msg = None;
                true
            }
            Msg::SetStatus(msg) => {
                self.status_msg = Some(msg);
                true
            }
            Msg::TokenReceived(token) => {
                if let Some(tokens) = self.tokens.as_mut() {
                    tokens.push_str(&token);
                } else {
                    self.tokens = Some(token.to_owned());
                }
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
                        responsive={true}
                    />
                }
            })
            .collect::<Html>();

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
                <div class="flex p-2">{self.status_msg.clone().unwrap_or_default()}</div>
                {if let Some(query) = &self.current_query {
                    html! { <div class="mb-2 px-6 text-2xl font-semibold text-white">{query}</div> }
                } else { html! {}}}
                <div class="grid w-full grid-cols-2 gap-8 px-6 py-4">
                    {if let Some(tokens) = &self.tokens {
                        html! {
                            <AnswerSection
                                tokens={tokens.clone()}
                                in_progress={self.in_progress}
                            />
                        }
                    } else {
                        html! {}
                    }}
                    <div class="animate-fade-in col-span-1">
                        {if !self.results.is_empty() {
                            html! {
                                <>
                                    <div class="mb-2 text-sm font-semibold uppercase text-cyan-500">{"Sources"}</div>
                                    {html}
                                </>
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                </div>
            </div>
        }
    }
}

#[derive(Properties, PartialEq)]
struct AnswerSectionProps {
    pub tokens: String,
    #[prop_or_default]
    pub in_progress: bool,
}

#[function_component(AnswerSection)]
fn answer_section(props: &AnswerSectionProps) -> Html {
    let html = markdown::to_html(&props.tokens.clone());
    let html = html.trim_start_matches("<p>").to_string();
    let html = html.trim_end_matches("</p>").to_string();
    let html = format!("<span>{}</span>", html);

    html! {
        <div class="animate-fade-in col-span-1">
            <div class="mb-2 text-sm font-semibold uppercase text-cyan-500">{"Answer"}</div>
            <div>
                <p class="prose prose-invert inline">
                    {Html::from_html_unchecked(AttrValue::from(html))}
                    { if props.in_progress {
                        html! { <div class="inline-block h-5 w-2 animate-pulse-fast bg-cyan-600 mb-[-4px]"></div> }
                    } else {
                        html! { <span>{"🔭"}</span>}
                    }}
                </p>
            </div>
        </div>
    }
}
