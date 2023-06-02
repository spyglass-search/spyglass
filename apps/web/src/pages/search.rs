use crate::{
    client::{ApiClient, ApiError, Lens, SpyglassClient},
    AuthStatus, Route,
};
use futures::lock::Mutex;
use shared::response::SearchResult;
use shared::{
    keyboard::KeyCode,
    response::{ChatErrorType, ChatUpdate},
};
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
use yew::platform::pinned::mpsc;
use yew::{html::Scope, platform::pinned::mpsc::UnboundedSender, prelude::*};
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
    Focus,
    SetChatUuid(String),
    ContextAdded(String),
    HandleFollowup(String),
    HandleKeyboardEvent(KeyboardEvent),
    HandleSearch,
    Reload,
    ReloadSavedSession(bool),
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
    pub session_uuid: String,
    pub chat_session: Option<String>,
    pub embedded: bool,
    pub lens_data: Option<Lens>,
}

#[derive(Clone, Debug)]
pub enum WorkerCmd {
    Stop,
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
    chat_uuid: Option<String>,
    session_uuid: String,
    historical_chat: bool,
    embedded: bool,
    _worker_cmd: Option<UnboundedSender<WorkerCmd>>,
    _context_listener: ContextHandle<AuthStatus>,
}

impl Component for SearchPage {
    type Message = Msg;
    type Properties = SearchPageProps;

    fn create(ctx: &yew::Context<Self>) -> Self {
        let props = ctx.props();

        let (auth_status, context_listener) = ctx
            .link()
            .context(ctx.link().callback(Msg::UpdateContext))
            .expect("No Message Context Provided");

        if props.chat_session.is_some() {
            ctx.link().send_message(Msg::ReloadSavedSession(true));
        } else if props.lens_data.is_none() {
            ctx.link().send_message(Msg::Reload);
        }

        {
            let link = ctx.link().clone();
            let timeout =
                gloo::timers::callback::Timeout::new(1_000, move || link.send_message(Msg::Focus));
            timeout.forget();
        }

        Self {
            client: Arc::new(Mutex::new(SpyglassClient::new(
                props.lens.clone(),
                props.session_uuid.clone(),
                auth_status.token.clone(),
                props.embedded,
            ))),
            embedded: props.embedded,
            auth_status,
            context: None,
            current_query: None,
            history: Vec::new(),
            in_progress: false,
            lens_data: props.lens_data.clone(),
            lens_identifier: props.lens.clone(),
            results: Vec::new(),
            search_input_ref: Default::default(),
            search_wrapper_ref: Default::default(),
            show_context: false,
            status_msg: None,
            tokens: None,
            chat_uuid: props.chat_session.clone(),
            session_uuid: props.session_uuid.clone(),
            historical_chat: props.chat_session.is_some(),
            _context_listener: context_listener,
            _worker_cmd: None,
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        if self.in_progress {
            ctx.link().send_message(Msg::StopSearch);
        }

        let new_lens = ctx.props().lens.clone();
        let new_chat_session = ctx.props().chat_session.clone();
        self.historical_chat = new_chat_session.is_some();

        let lens_changed = self.lens_identifier != new_lens;
        let chat_session_changed = self.chat_uuid != new_chat_session;
        let chat_session_set = new_chat_session.is_some();

        if lens_changed {
            self.lens_identifier = new_lens;
            if chat_session_set && chat_session_changed {
                self.chat_uuid = new_chat_session;
                ctx.link().send_message(Msg::ReloadSavedSession(true));
            } else {
                ctx.link().send_message(Msg::Reload);
            }
            true
        } else if chat_session_changed {
            if chat_session_set {
                self.chat_uuid = new_chat_session;
                ctx.link().send_message(Msg::ReloadSavedSession(false));
            } else {
                ctx.link().send_message(Msg::Reload);
            }

            true
        } else {
            false
        }
    }

    fn update(&mut self, ctx: &yew::Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::Focus => {
                if let Some(search_input) = self.search_input_ref.cast::<HtmlInputElement>() {
                    let _ = search_input.focus();
                }
                true
            }
            Msg::SetChatUuid(uuid) => {
                self.chat_uuid = Some(uuid);
                false
            }
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
                self.context = None;
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

                self.tokens = None;
                self.status_msg = None;
                self.context = None;
                self.in_progress = true;

                let link = link.clone();

                let cur_doc_context = self.results.clone();
                let chat_uuid = self.chat_uuid.clone();
                let client = self.client.clone();
                let (tx, rx) = mpsc::unbounded::<WorkerCmd>();
                self._worker_cmd = Some(tx);
                spawn_local(async move {
                    let mut client = client.lock().await;
                    if let Err(err) = client
                        .followup(
                            &question,
                            &cur_history,
                            &cur_doc_context,
                            &chat_uuid,
                            &{
                                let link = link.clone();
                                move |update| process_update(update, &link)
                            },
                            rx,
                        )
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
                    self.reset_search();
                    let query = search_input.value();
                    link.send_message(Msg::SetQuery(query));
                    search_input.set_value("");
                }
                false
            }
            Msg::ReloadSavedSession(full_reload) => {
                if self.chat_uuid.is_some() {
                    let chat_uuid = self.chat_uuid.clone().unwrap();
                    if full_reload {
                        self.reload(link);
                    } else {
                        self.reset_search();
                    }

                    self.chat_uuid = Some(chat_uuid.clone());
                    if let Some(data) = &self.auth_status.user_data {
                        if let Some(history) = data
                            .history
                            .iter()
                            .find(|history| history.session_id == chat_uuid)
                        {
                            let mut first_question = None;
                            for qna in &history.qna {
                                if first_question.is_none() {
                                    first_question = Some(qna.question.clone());
                                }
                                self.history.push(HistoryItem {
                                    source: HistorySource::User,
                                    value: qna.question.clone(),
                                });
                                self.history.push(HistoryItem {
                                    source: HistorySource::Clippy,
                                    value: qna.response.clone(),
                                });

                                if let Some(doc_details) = &qna.document_details {
                                    link.send_message(Msg::SetSearchResults(doc_details.clone()))
                                }
                            }
                            self.current_query = first_question;
                        }
                    }
                } else {
                    self.reload(link);
                }
                true
            }
            Msg::Reload => {
                self.reload(link);
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
                    if let Err(err) = client
                        .search(
                            &query,
                            &{
                                let link = link.clone();
                                move |update| process_update(update, &link)
                            },
                            rx,
                        )
                        .await
                    {
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
                    tx.close_now();
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
                if self.chat_uuid.is_some() {
                    link.send_message(Msg::ReloadSavedSession(true))
                } else {
                    link.send_message(Msg::Reload);
                }
                false
            }
        }
    }

    fn view(&self, ctx: &yew::Context<Self>) -> yew::Html {
        let link = ctx.link();
        if let Some(lens) = self.lens_data.clone() {
            html! {
                <>
                    {self.render_search(link, &lens)}
                    {if !self.auth_status.is_authenticated {
                        html! {
                        <div class="sticky top-[90vh] md:top-[100vh] mx-auto w-fit text-center pb-4">
                            <a href="/" class="flex cursor-pointer flex-row items-center rounded-full bg-cyan-700 px-2 md:px-4 py-1 md:py-2 hover:bg-cyan-900">
                                <img src="/icons/logo@2x.png" class="w-6 md:w-8" />
                                <div class="ml-1 md:ml-2 text-left">
                                    <div class="text-xs md:text-sm font-semibold md:font-bold">{"Powered by Spyglass"}</div>
                                    <div class="hidden md:block text-xs text-cyan-200">{"Click to create your own"}</div>
                                </div>
                            </a>
                            <div class="hidden md:block mt-4 text-sm text-neutral-500">{"Made with ☕️ in SF/SD"}</div>
                        </div>
                        }
                    } else { html! {} }}
                </>
            }
        } else {
            html! {}
        }
    }
}

fn process_update(update: ChatUpdate, link: &Scope<SearchPage>) {
    match update {
        ChatUpdate::ChatStart(uuid) => {
            log::info!("ChatUpdate::ChatStart");
            link.send_message(Msg::SetChatUuid(uuid))
        }
        ChatUpdate::SearchingDocuments => {
            log::info!("ChatUpdate::SearchingDocuments");
            link.send_message(Msg::SetStatus("Searching...".into()))
        }
        ChatUpdate::DocumentContextAdded(docs) => {
            log::info!("ChatUpdate::DocumentContextAdded");
            link.send_message(Msg::SetSearchResults(docs))
        }
        ChatUpdate::GeneratingContext => {
            log::info!("ChatUpdate::SearchingDocuments");
            link.send_message(Msg::SetStatus("Analyzing documents...".into()))
        }
        ChatUpdate::ContextGenerated(context) => {
            log::info!("ChatUpdate::ContextGenerated {}", context);
            link.send_message(Msg::ContextAdded(context));
        }
        ChatUpdate::LoadingModel | ChatUpdate::LoadingPrompt => {
            link.send_message(Msg::SetStatus("Generating answer...".into()))
        }
        ChatUpdate::Token(token) => link.send_message(Msg::TokenReceived(token)),
        ChatUpdate::EndOfText => {
            link.send_message(Msg::SetFinished);
        }
        ChatUpdate::Error(err) => {
            log::error!("ChatUpdate::Error: {err:?}");
            let msg = match err {
                ChatErrorType::ContextLengthExceeded(msg) => msg,
                ChatErrorType::APIKeyMissing => "No API key".into(),
                ChatErrorType::UnknownError(msg) => msg,
            };
            link.send_message(Msg::SetError(msg));
        }
    }
}

impl SearchPage {
    fn reset_search(&mut self) {
        self.context = None;
        self.current_query = None;
        self.history.clear();
        self.in_progress = false;
        self.results.clear();
        self.status_msg = None;
        self.tokens = None;
        self.chat_uuid = None;
    }

    fn render_search(&self, link: &Scope<SearchPage>, lens: &Lens) -> Html {
        let placeholder = format!("Ask anything related to \"{}\"", lens.display_name);

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

        let nav_link = link.clone();
        let lens_id = self.lens_identifier.clone();
        let nav_callback = Callback::from(move |_| {
            nav_link.navigator().unwrap().push(&Route::Search {
                lens: lens_id.clone(),
            })
        });

        html! {
            <div ref={self.search_wrapper_ref.clone()}>
                <div class="p-2 md:p-8 flex flex-row items-center gap-4 pb-10 md:pb-14">
                    {if let Some(image) = lens.image.clone() {
                        html! {
                            <div class="flex-none">
                                <img class="rounded h-12 md:h-24 w-12 md:w-24"  src={image}/>
                            </div>
                        }
                    } else { html! {} }}
                    <div class="self-start md:self-end py-0 md:py-2">
                        <div class="font-bold text-base md:text-2xl">{lens.display_name.clone()}</div>
                        {if let Some(desc) = lens.description.clone() {
                            html! {
                                <div class="text-xs md:text-sm text-neutral-400 w-full md:w-3/4 h-8 md:h-fit overflow-hidden">
                                    {desc}
                                </div>
                            }
                        } else { html! {} }}
                    </div>
                </div>
                {if !self.historical_chat {
                    html! {
                    <div class="flex flex-nowrap w-full px-4 md:px-8 -mt-6 md:-mt-8">
                        <input
                            ref={self.search_input_ref.clone()}
                            id="searchbox"
                            type="text"
                            class="flex-1 overflow-hidden bg-white rounded-l p-2 md:p-4 text-base md:text-2xl text-black placeholder-neutral-300 caret-black outline-none focus:outline-none active:outline-none"
                            placeholder={placeholder}
                            spellcheck="false"
                            tabindex="-1"
                            onkeyup={link.callback(Msg::HandleKeyboardEvent)}
                            autofocus={true}
                        />
                        <div class="p-1 md:p-2 bg-white rounded-r">
                            {if self.in_progress {
                                html! {
                                    <Btn
                                        _type={BtnType::Borderless}
                                        classes="rounded p-2 md:p-4 bg-cyan-600 hover:bg-cyan-800"
                                        onclick={link.callback(|_| Msg::StopSearch)}
                                    >
                                        <RefreshIcon animate_spin={true} height="h-5" width="w-5" classes={"text-white mr-2"} />
                                        {"Stop"}
                                    </Btn>
                                }
                            } else {
                                html! {
                                    <Btn
                                        _type={BtnType::Borderless}
                                        classes="rounded p-2 md:p-4 bg-cyan-600 hover:bg-cyan-800"
                                        onclick={link.callback(|_| Msg::HandleSearch)}
                                    >
                                        <SearchIcon width="w-6" height="h-6" />
                                    </Btn>

                                }
                            }}
                        </div>
                    </div>
                    }
                }
                else {
                    html! {
                        <Btn _type={BtnType::Primary} onclick={nav_callback} classes="mx-8 flex-1">
                            {"New Chat"}
                        </Btn>
                    }
                }}
                {if self.show_context {
                    html! {
                        <div class="px-8 py-6">
                            <div class="mb-2 text-sm font-semibold uppercase text-cyan-500">{"Context"}</div>
                            <pre class="text-sm">{self.context.clone()}</pre>
                        </div>
                    }
                } else {
                    html! {}
                }}
                {if let Some(query) = &self.current_query {
                    html! { <div class="mt-6 md:mt-8 px-4 md:px-8 text-lg md:text-2xl font-semibold text-white">{query}</div> }
                } else { html! {}}}
                <div class="lg:grid lg:grid-cols-2 flex flex-col w-full gap-8 p-4 md:p-8">
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
                    } else if !lens.example_questions.is_empty() {
                        html! {
                            <FAQComponent
                                questions={lens.example_questions.clone()}
                                onclick={link.callback(Msg::SetQuery)}
                            />
                        }
                    } else {
                        html! {}
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

    // Fully reloads and resets the search context. This is used when the lens has changed.
    fn reload(&mut self, link: &Scope<SearchPage>) {
        self.reset_search();
        self.client = Arc::new(Mutex::new(SpyglassClient::new(
            self.lens_identifier.clone(),
            self.session_uuid.clone(),
            self.auth_status.token.clone(),
            self.embedded,
        )));

        let auth_status = self.auth_status.clone();
        let identifier = self.lens_identifier.clone();
        let link = link.clone();
        let embedded = self.embedded;
        spawn_local(async move {
            let api = if embedded {
                ApiClient::new(None, true)
            } else {
                auth_status.get_client()
            };

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
            <div class="flex flex-col text-sm md:text-base leading-relaxed">
                <div class="flex flex-col gap-4">
                    <HistoryLog history={props.history.clone()} />
                    { if let Some(tokens) = &props.tokens {
                        html! { <HistoryLogItem source={HistorySource::Clippy} tokens={tokens.clone()} in_progress={props.in_progress} /> }
                    } else if let Some(msg) = &props.status {
                        html! { <HistoryLogItem source={HistorySource::System} tokens={msg.clone()}  /> }
                    } else {
                        html! {}
                    }}
                </div>
                <form class="flex flex-row" onsubmit={on_ask_followup}>
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
        classes!(
            "prose-sm",
            "md:prose-base",
            "prose",
            "prose-invert",
            "inline"
        )
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
        "text-base",
        "md:text-lg",
        "p-2",
        "md:p-4",
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
        <div class="col-span-2 mx-auto pt-4">
            <div class="text-base md:text-xl text-white font-semibold">{"Example Questions"}</div>
            <div class="text-neutral-500 text-sm md:text-base">
                {"Not sure where to start? Try one of these questions"}
            </div>
            <div class="flex flex-col gap-4 mt-4">
                {questions}
            </div>
        </div>
    }
}
