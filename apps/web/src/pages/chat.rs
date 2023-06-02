use crate::{
    client::{ApiClient, ApiError, Lens, SpyglassClient},
    components::chat_bubble::{self, ChatBubble},
    pages::search::{HistorySource, WorkerCmd},
    schema::Theme,
    utils::validate_hex_color,
    AuthStatus,
};
use futures::lock::Mutex;
use shared::response::SearchResult;
use shared::{
    keyboard::KeyCode,
    response::{ChatErrorType, ChatUpdate},
};
use std::str::FromStr;
use std::sync::Arc;
use ui_components::{
    btn::{Btn, BtnType},
    icons::{RefreshIcon, SearchIcon},
};
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::platform::pinned::mpsc;
use yew::{html::Scope, platform::pinned::mpsc::UnboundedSender, prelude::*};
use yew_router::prelude::*;

use super::search::HistoryItem;

// make sure we only have one connection per client
type Client = Arc<Mutex<SpyglassClient>>;

#[allow(dead_code)]
pub enum Msg {
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
pub struct ChatPageProps {
    pub lens: String,
    pub session_uuid: String,
    pub chat_session: Option<String>,
    pub lens_data: Option<Lens>,
}

pub struct ChatPage {
    client: Client,
    lens_identifier: String,
    lens_data: Option<Lens>,
    auth_status: AuthStatus,
    current_query: Option<String>,
    history: Vec<HistoryItem>,
    in_progress: bool,
    results: Vec<SearchResult>,
    search_input_ref: NodeRef,
    status_msg: Option<String>,
    tokens: Option<String>,
    context: Option<String>,
    show_context: bool,
    chat_uuid: Option<String>,
    session_uuid: String,
    historical_chat: bool,
    _worker_cmd: Option<UnboundedSender<WorkerCmd>>,
    _context_listener: ContextHandle<AuthStatus>,
}

impl Component for ChatPage {
    type Message = Msg;
    type Properties = ChatPageProps;

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

        Self {
            client: Arc::new(Mutex::new(SpyglassClient::new(
                props.lens.clone(),
                props.session_uuid.clone(),
                auth_status.token.clone(),
                true,
            ))),
            auth_status,
            context: None,
            current_query: None,
            history: Vec::new(),
            in_progress: false,
            lens_data: props.lens_data.clone(),
            lens_identifier: props.lens.clone(),
            results: Vec::new(),
            search_input_ref: Default::default(),
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
                    let query = search_input.value();
                    if self.current_query.is_some() {
                        link.send_message(Msg::HandleFollowup(query))
                    } else {
                        link.send_message(Msg::SetQuery(query));
                    }
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
                <div class="h-screen w-full">
                  {self.render_search(link, &lens)}
                </div>
            }
        } else {
            html! {}
        }
    }
}

fn process_update(update: ChatUpdate, link: &Scope<ChatPage>) {
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

impl ChatPage {
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

    fn render_search(&self, link: &Scope<ChatPage>, lens: &Lens) -> Html {
        let placeholder = format!("Ask anything related to {}", lens.display_name);

        let mut chats = Vec::new();
        let mut header_color = String::from("#454545");
        let bot_bubble_color = lens
            .embedded_configuration
            .as_ref()
            .map(|cfg| cfg.bot_bubble_color.clone())
            .unwrap_or_default();
        let user_bubble_color = lens
            .embedded_configuration
            .as_ref()
            .map(|cfg| cfg.user_bubble_color.clone())
            .unwrap_or_default();
        let mut theme = "dark-mode";
        let mut header_title = lens.display_name.clone();
        let add_initial: bool = lens
            .embedded_configuration
            .as_ref()
            .map(|cfg| cfg.initial_chat.is_empty())
            .unwrap_or(true);

        if add_initial {
            chats.push(html! {
                <ChatBubble background={bot_bubble_color.clone()} align={chat_bubble::ChatAlign::Left} text={placeholder.clone()}/>
            });
        }

        if let Some(embedding_config) = &lens.embedded_configuration {
            // Add initial chat messages
            for initial in &embedding_config.initial_chat {
                chats.push(html! {
                    <ChatBubble background={bot_bubble_color.clone()} align={chat_bubble::ChatAlign::Left} text={initial.clone()}/>
                });
            }

            if let Some(color) = &embedding_config.header_color {
                if validate_hex_color(color).is_ok() {
                    header_color = format!("#{}", color);
                }
            }

            if let Some(title) = &embedding_config.header_title {
                header_title = title.clone();
            }

            theme = match &embedding_config.theme {
                Theme::LightMode => "light-mode",
                Theme::DarkMode => "dark-mode",
            }
        }

        log::error!("history {:?}", self.history.len());

        chats.extend(self
            .history
            .iter()
            .filter_map(|history| match history.source {
                HistorySource::Clippy => Some(html! {
                    <ChatBubble background={bot_bubble_color.clone()} align={chat_bubble::ChatAlign::Left} text={history.value.clone()}/>
                }),
                HistorySource::User => Some(html! {
                    <ChatBubble background={user_bubble_color.clone()} align={chat_bubble::ChatAlign::Right} text={history.value.clone()}/>
                }),
                _ => None,
            })
            .collect::<Vec<Html>>());

        // TODO configure justify and background color
        let justify = classes!("justify-center", "flex", "items-center", "mb-4", "p-4");

        let chat_display = classes!(theme, "h-full");
        // TODO configurable product icon, bot image, user image, etc
        html! {
            <div class={chat_display}>
                <div class="bg-[var(--spy-primary-background)] shadow rounded-lg flex flex-col h-full">
                  <div class={justify} style={header_color}>
                      <div class="ml-2">
                        <h2 class="text-lg font-bold">{header_title}</h2>
                        //   <!-- p class="text-sm text-gray-500" -->{"Online"}</p -->
                      </div>
                  </div>
                <div class="flex-grow overflow-y-auto p-4">
                  <div class="flex flex-col space-y-2">
                  {chats}
                   { if self.history.is_empty() {
                      if let Some(query) = &self.current_query {
                         html!{ <ChatBubble background={user_bubble_color.clone()} align={chat_bubble::ChatAlign::Right} text={query.clone()}  /> }
                       } else {
                        html! {}
                       }
                    } else {
                        html! {}

                    }}
                    { if let Some(tokens) = &self.tokens {
                        html!{ <ChatBubble background={bot_bubble_color.clone()} align={chat_bubble::ChatAlign::Left} text={tokens.clone()}  /> }
                    } else if let Some(msg) = &self.status_msg {
                        html!{ <ChatBubble background={bot_bubble_color.clone()} align={chat_bubble::ChatAlign::Left} text={msg.clone()}  /> }
                    } else {
                        html! {}
                    }}
                  </div>
                </div>
                <div class="mt-4 p-4">
                    <div class="flex rounded-lg shadow-sm">
                    <input
                        ref={self.search_input_ref.clone()}
                        id="searchbox"
                        type="text"
                        placeholder="Type your message..."
                        class="text-black flex-1 border-gray-200 rounded-l-lg p-2 focus:outline-none focus:ring-2 focus:ring-blue-200"
                        placeholder={self.current_query.clone().unwrap_or(placeholder)}
                        spellcheck="false"
                        tabindex="-1"
                        onkeyup={link.callback(Msg::HandleKeyboardEvent)}/>
                    {if self.in_progress {
                        html! {
                            <Btn
                                _type={BtnType::Borderless}
                                classes="rounded-r px-8 bg-cyan-600 hover:bg-cyan-800"
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
                                classes="rounded-r px-8 bg-cyan-600 hover:bg-cyan-800"
                                onclick={link.callback(|_| Msg::HandleSearch)}
                            >
                                <SearchIcon width="w-6" height="h-6" />
                            </Btn>

                        }
                    }}
                </div>
                </div>
                <div class="mx-auto w-fit text-center pb-2">
                        <a href="https://search.spyglass.fyi/" class="flex cursor-pointer flex-row items-center rounded-full bg-cyan-700 px-2 py-1 hover:bg-cyan-900">
                            <img src="/icons/logo@2x.png" class="w-4" />
                            <div class="ml-2 text-left">
                                <div class="text-sm font-bold">{"Powered by Spyglass"}</div>
                                <div class="text-xs text-cyan-200">{"Click to create your own"}</div>
                            </div>
                        </a>
                        <div class="mt-2 text-sm text-neutral-500">{"Made with ☕️ in SF/SD"}</div>
                    </div>
                </div>
          </div>
        }
    }

    // Fully reloads and resets the search context. This is used when the lens has changed.
    fn reload(&mut self, link: &Scope<ChatPage>) {
        self.reset_search();
        self.client = Arc::new(Mutex::new(SpyglassClient::new(
            self.lens_identifier.clone(),
            self.session_uuid.clone(),
            self.auth_status.token.clone(),
            true,
        )));

        let identifier = self.lens_identifier.clone();
        let link = link.clone();
        spawn_local(async move {
            let api = ApiClient::new(None, true);
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
