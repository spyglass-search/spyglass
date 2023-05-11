use crate::{
    client::{ApiError, Lens, SpyglassClient},
    AuthStatus,
};
use futures::lock::Mutex;
use shared::keyboard::KeyCode;
use shared::response::SearchResult;
use std::str::FromStr;
use std::sync::Arc;
use strum_macros::Display;
use ui_components::{
    btn::{Btn, BtnType},
    icons::{RefreshIcon, SearchIcon},
    results::{ResultPaginator, WebSearchResultItem},
};
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::{html::Scope, prelude::*, platform::pinned::mpsc::UnboundedSender};
use yew::platform::pinned::mpsc;
use yew_router::prelude::*;

// make sure we only have one connection per client
type Client = Arc<Mutex<SpyglassClient>>;

#[derive(Clone, PartialEq, Eq, Display)]
pub enum HistorySource {
    #[strum(serialize = "assistant")]
    Clippy,
    #[strum(serialize = "user")]
    User,
    #[strum(serialize = "system")]
    System,
}

#[derive(Clone, PartialEq, Eq)]
pub struct HistoryItem {
    /// who "wrote" this response
    pub source: HistorySource,
    pub value: String,
}

#[allow(dead_code)]
pub enum Msg {
    ContextAdded(String),
    HandleFollowup(String),
    HandleKeyboardEvent(KeyboardEvent),
    HandleSearch,
    Reload,
    SetError(String),
    SetFinished,
    SetLensData(Lens),
    SetQuery(String),
    SetSearchResults(Vec<SearchResult>),
    SetStatus(String),
    StopSearch,
    ToggleContext,
    TokenReceived(String),
    UpdateContext(AuthStatus),
}

#[derive(Properties, PartialEq)]
pub struct SearchPageProps {
    pub lens: String,
}

#[derive(Clone, Debug)]
pub enum WorkerCmd {
    Stop
}

pub struct SearchPage {
    client: Client,
    lens_identifier: String,
    lens_data: Option<Lens>,
    auth_status: AuthStatus,
    current_query: Option<String>,
    history: Vec<HistoryItem>,
    in_progress: bool,
    results: Vec<SearchResult>,
    search_input_ref: NodeRef,
    search_wrapper_ref: NodeRef,
    status_msg: Option<String>,
    tokens: Option<String>,
    context: Option<String>,
    show_context: bool,
    _worker_cmd: Option<UnboundedSender<WorkerCmd>>,
    _context_listener: ContextHandle<AuthStatus>,
}

impl Component for SearchPage {
    type Message = Msg;
    type Properties = SearchPageProps;

    fn create(ctx: &yew::Context<Self>) -> Self {
        let props = ctx.props();
        let link = ctx.link();
        link.send_message(Msg::Reload);

        let (auth_status, context_listener) = ctx
            .link()
            .context(ctx.link().callback(Msg::UpdateContext))
            .expect("No Message Context Provided");

        ctx.link().send_message(Msg::Reload);

        Self {
            auth_status,
            client: Arc::new(Mutex::new(SpyglassClient::new(props.lens.clone()))),
            context: None,
            current_query: None,
            history: Vec::new(),
            in_progress: false,
            lens_data: None,
            lens_identifier: props.lens.clone(),
            results: Vec::new(),
            search_input_ref: Default::default(),
            search_wrapper_ref: Default::default(),
            show_context: false,
            status_msg: None,
            tokens: None,
            _context_listener: context_listener,
            _worker_cmd: None
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        let new_lens = ctx.props().lens.clone();
        if self.lens_identifier != new_lens {
            self.lens_identifier = new_lens;
            ctx.link().send_message(Msg::Reload);
            true
        } else {
            false
        }
    }

    fn update(&mut self, ctx: &yew::Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::ContextAdded(context) => {
                self.context = Some(context);
                false
            }
            Msg::HandleFollowup(question) => {
                log::info!("handling followup: {}", question);
                // Push existing question & answer into history
                if let Some(value) = &self.current_query {
                    self.history.push(HistoryItem {
                        source: HistorySource::User,
                        value: value.to_owned(),
                    });
                }

                // Push existing answer into history
                if let Some(value) = &self.tokens {
                    self.history.push(HistoryItem {
                        source: HistorySource::Clippy,
                        value: value.to_owned(),
                    });
                }

                // Push user's question into history
                self.history.push(HistoryItem {
                    source: HistorySource::User,
                    value: question.clone(),
                });

                self.tokens = None;
                self.status_msg = None;
                self.context = None;
                self.in_progress = true;

                let link = link.clone();
                let mut cur_history = self.history.clone();
                // Add context to the beginning
                if let Some(context) = &self.context {
                    cur_history.insert(
                        0,
                        HistoryItem {
                            source: HistorySource::User,
                            value: context.to_owned(),
                        },
                    );
                }

                let cur_doc_context = self.results.clone();
                let client = self.client.clone();
                let (tx, rx) = mpsc::unbounded::<WorkerCmd>();
                self._worker_cmd = Some(tx);
                spawn_local(async move {
                    let mut client = client.lock().await;
                    if let Err(err) = client
                        .followup(&question, &cur_history, &cur_doc_context, link.clone(), rx)
                        .await
                    {
                        log::error!("{}", err.to_string());
                        link.send_message(Msg::SetError(err.to_string()));
                    }
                });

                true
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
                if let Some(search_input) = self.search_input_ref.cast::<HtmlInputElement>() {
                    let query = search_input.value();
                    link.send_message(Msg::SetQuery(query));
                    search_input.set_value("");
                }
                false
            }
            Msg::Reload => {
                self.context = None;
                self.current_query = None;
                self.history.clear();
                self.in_progress = false;
                self.results.clear();
                self.status_msg = None;
                self.tokens = None;

                let auth_status = self.auth_status.clone();
                let identifier = self.lens_identifier.clone();
                let link = link.clone();
                spawn_local(async move {
                    let api = auth_status.get_client();
                    match api.lens_retrieve(&identifier).await {
                        Ok(lens) => link.send_message(Msg::SetLensData(lens)),
                        Err(ApiError::ClientError(msg)) => {
                            // Unauthorized
                            if msg.code == 400 {
                                let navi = link.navigator().expect("No navigator");
                                navi.push(&crate::Route::Start);
                            }
                            log::error!("error retrieving lens: {msg}");
                        }
                        Err(err) => log::error!("error retrieving lens: {}", err),
                    }
                });

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
            Msg::SetLensData(data) => {
                self.lens_data = Some(data);
                true
            }
            Msg::SetSearchResults(results) => {
                self.results = results;
                true
            }
            Msg::SetStatus(msg) => {
                self.status_msg = Some(msg);
                true
            }
            Msg::SetQuery(query) => {
                self.in_progress = true;
                self.tokens = None;
                self.results = Vec::new();
                self.current_query = Some(query.clone());

                log::info!("handling search! {}", query);
                self.status_msg = Some(format!("searching: {query}"));

                let link = link.clone();
                let client = self.client.clone();

                let (tx, rx) = mpsc::unbounded::<WorkerCmd>();
                self._worker_cmd = Some(tx);
                spawn_local(async move {
                    let mut client = client.lock().await;
                    if let Err(err) = client.search(&query, link.clone(), rx).await {
                        log::error!("{}", err.to_string());
                        link.send_message(Msg::SetError(err.to_string()));
                    } else {
                        log::info!("finished response");
                    }
                });

                true
            }
            Msg::StopSearch => {
                if let Some(tx) = &self._worker_cmd {
                    self.in_progress = false;
                    let _ = tx.send_now(WorkerCmd::Stop);
                    let _ = tx.close_now();
                    self._worker_cmd = None;
                }
                true
            }
            Msg::ToggleContext => {
                self.show_context = !self.show_context;
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
            Msg::UpdateContext(auth) => {
                self.auth_status = auth;
                false
            }
        }
    }

    fn view(&self, ctx: &yew::Context<Self>) -> yew::Html {
        let link = ctx.link();
        if let Some(lens) = self.lens_data.clone() {
            self.render_search(link, &lens)
        } else {
            html! {}
        }
    }
}

impl SearchPage {
    fn render_search(&self, link: &Scope<SearchPage>, lens: &Lens) -> Html {
        let placeholder = format!("Ask anything related to {}", lens.display_name);

        let results = self
            .results
            .iter()
            .map(|result| {
                html! {
                    <WebSearchResultItem
                        id={format!("result-{}", result.doc_id)}
                        result={result.clone()}
                    />
                }
            })
            .collect::<Vec<Html>>();

        html! {
            <div ref={self.search_wrapper_ref.clone()} class="relative">
                <div class="flex flex-nowrap w-full bg-neutral-800 p-4 border-b-2 border-neutral-900">
                    <input
                        ref={self.search_input_ref.clone()}
                        id="searchbox"
                        type="text"
                        class="bg-neutral-800 text-white text-2xl py-3 overflow-hidden flex-1 outline-none active:outline-none focus:outline-none caret-white placeholder-neutral-600"
                        placeholder={self.current_query.clone().unwrap_or(placeholder)}
                        spellcheck="false"
                        tabindex="-1"
                        onkeyup={link.callback(Msg::HandleKeyboardEvent)}
                    />
                    {if self.in_progress {
                        html! {
                            <Btn
                                _type={BtnType::Primary}
                                onclick={link.callback(|_| Msg::StopSearch)}
                            >
                                <RefreshIcon animate_spin={true} height="h-5" width="w-5" classes={"text-white"} />
                                {"Stop"}
                            </Btn>
                        }
                    } else {
                        html! {
                            <Btn
                                _type={BtnType::Primary}
                                onclick={link.callback(|_| Msg::HandleSearch)}
                            >
                                <SearchIcon width="w-6" height="h-6" />
                            </Btn>

                        }
                    }}
                </div>
                {if self.show_context {
                    html! {
                        <div class="p-4">
                            <div>{"Context"}</div>
                            <div>{self.context.clone()}</div>
                        </div>
                    }
                } else {
                    html! {}
                }}
                {if let Some(query) = &self.current_query {
                    html! { <div class="mt-4 mb-2 px-6 text-2xl font-semibold text-white">{query}</div> }
                } else { html! {}}}
                <div class="lg:grid lg:grid-cols-2 flex flex-col w-full gap-8 px-6 py-4">
                    { if !self.history.is_empty() || self.tokens.is_some() || self.status_msg.is_some() {
                        html! {
                            <AnswerSection
                                history={self.history.clone()}
                                tokens={self.tokens.clone()}
                                status={self.status_msg.clone()}
                                in_progress={self.in_progress}
                                on_followup={link.callback(Msg::HandleFollowup)}
                            />
                        }
                    } else {
                        html! {
                            <FAQComponent
                                questions={lens.example_questions.clone()}
                                onclick={link.callback(Msg::SetQuery)}
                            />
                        }
                    }}

                    <div class="animate-fade-in col-span-1">
                        {if !results.is_empty() {
                            html! {
                                <>
                                    <div class="mb-2 text-sm font-semibold uppercase text-cyan-500">{"Sources"}</div>
                                    <ResultPaginator page_size={5}>{results}</ResultPaginator>
                                </>
                            }
                        } else if self.current_query.is_some() {
                            html! {
                                <>
                                    <div class="mb-2 text-sm font-semibold uppercase text-cyan-500">{"Sources"}</div>
                                    <div class="text-sm text-neutral-500">
                                        {
                                            "We didn't find any relevant documents, but we
                                            will try to answer the question to the best of our ability
                                            without any additional context"
                                        }
                                    </div>
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
    pub history: Vec<HistoryItem>,
    pub tokens: Option<String>,
    pub status: Option<String>,
    #[prop_or_default]
    pub in_progress: bool,
    #[prop_or_default]
    pub on_followup: Callback<String>,
}

#[function_component(AnswerSection)]
fn answer_section(props: &AnswerSectionProps) -> Html {
    let ask_followup = use_node_ref();
    let ask_followup_handle = ask_followup.clone();
    let on_followup_cb = props.on_followup.clone();
    let on_ask_followup = Callback::from(move |event: SubmitEvent| {
        event.prevent_default();
        if let Some(node) = ask_followup_handle.cast::<HtmlInputElement>() {
            on_followup_cb.emit(node.value());
            node.set_value("");
        }
    });

    html! {
        <div class="animate-fade-in col-span-1">
            <div class="mb-2 text-sm font-semibold uppercase text-cyan-500">{"Answer"}</div>
            <div class="flex flex-col">
                <div class="flex flex-col min-h-[480px] gap-4">
                    <HistoryLog history={props.history.clone()} />
                    { if let Some(tokens) = &props.tokens {
                        html!{ <HistoryLogItem source={HistorySource::Clippy} tokens={tokens.clone()} in_progress={props.in_progress} /> }
                    } else if let Some(msg) = &props.status {
                        html!{ <HistoryLogItem source={HistorySource::System} tokens={msg.clone()}  /> }
                    } else {
                        html! {}
                    }}
                </div>
                <form class="mt-8 flex flex-row px-8" onsubmit={on_ask_followup}>
                    <textarea ref={ask_followup}
                        disabled={props.in_progress}
                        rows="3"
                        placeholder="Ask a followup question"
                        type="text"
                        class="w-full flex-1 border-b-2 border-neutral-600 bg-neutral-700 text-base text-white caret-white outline-none placeholder:text-gray-300 focus:outline-none active:outline-none p-4"
                    ></textarea>
                    <button
                        disabled={props.in_progress}
                        type="submit"
                        class="cursor-pointer items-center px-3 py-2 text-base font-semibold leading-5 bg-neutral-700 hover:bg-cyan-800"
                    >
                        <SearchIcon width="w-6" height="h-6" />
                    </button>
                </form>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct HistoryLogProps {
    pub history: Vec<HistoryItem>,
}

#[function_component(HistoryLog)]
fn history_log(props: &HistoryLogProps) -> Html {
    let html = props
        .history
        .iter()
        // Skip the initial question, we already show this at the top.
        .skip(1)
        .map(|item| {
            html! {
                <HistoryLogItem
                    source={item.source.clone()}
                    tokens={item.value.clone()}
                />
            }
        })
        .collect::<Html>();
    html! { <>{html}</> }
}

#[derive(Properties, PartialEq)]
struct HistoryLogItemProps {
    pub source: HistorySource,
    pub tokens: String,
    // Is this a item currently generating tokens?
    #[prop_or_default]
    pub in_progress: bool,
}

#[function_component(HistoryLogItem)]
fn history_log_item(props: &HistoryLogItemProps) -> Html {
    let user_icon = match props.source {
        HistorySource::Clippy | HistorySource::System => html! {<>{"🔭"}</>},
        HistorySource::User => html! {<>{"🧙‍♂️"}</>},
    };

    let html = markdown::to_html(&props.tokens.clone());
    let html = html.trim_start_matches("<p>").to_string();
    let html = html.trim_end_matches("</p>").to_string();
    let html = format!("<span>{}</span>", html);

    let item_classes = if props.source == HistorySource::User {
        classes!("text-white", "font-bold", "text-lg")
    } else {
        classes!("prose", "prose-invert", "inline")
    };

    html! {
        <div class="border-b border-neutral-600 pb-4">
            <p class={item_classes}>
                {Html::from_html_unchecked(AttrValue::from(html))}
                { if props.in_progress && props.source != HistorySource::User {
                    html! { <div class="inline-block h-5 w-2 animate-pulse-fast bg-cyan-600 mb-[-4px]"></div> }
                } else { html! {} }}
            </p>
            { if !props.in_progress && props.source != HistorySource::User {
                html! { <div class="mt-2">{user_icon}</div>}
            } else {
                html! {}
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct FAQComponentProps {
    questions: Vec<String>,
    #[prop_or_default]
    onclick: Callback<String>,
}

#[function_component(FAQComponent)]
fn faq_component(props: &FAQComponentProps) -> Html {
    let qa_classes = classes!(
        "text-cyan-500",
        "text-lg",
        "p-4",
        "rounded",
        "border",
        "border-neutral-500",
        "underline",
        "cursor-pointer",
        "hover:bg-neutral-700",
        "text-left",
    );

    let onclick = props.onclick.clone();
    let questions = props
        .questions
        .iter()
        .map(|q| {
            let onclick = onclick.clone();
            let question = q.clone();
            let callback = Callback::from(move |_| {
                onclick.emit(question.clone());
            });
            html! {
                <button class={qa_classes.clone()} onclick={callback}>{q.clone()}</button>
            }
        })
        .collect::<Html>();

    html! {
        <div>
            <div class="text-xl text-white">{"Frequently Asked Questions"}</div>
            <div class="text-neutral-500 text-base">
                {"Not sure where to start? Try one of these questions"}
            </div>
            <div class="flex flex-col gap-4 mt-4">
                {questions}
            </div>
        </div>
    }
}
