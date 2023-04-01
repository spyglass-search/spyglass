use gloo::console::console_dbg;
use serde_wasm_bindgen::from_value;
use shared::event::{self, ClientEvent, ListenPayload};
use shared::request::{AskClippyRequest, ClippyContext, LLMResponsePayload};
use shared::response::{DocMetadata, SendToAskClippyPayload};
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsValue;
use web_sys::{HtmlElement, HtmlInputElement};
use yew::html::Scope;
use yew::platform::spawn_local;
use yew::prelude::*;
use yew::NodeRef;

use crate::components::{btn, icons};
use crate::tauri_invoke;

#[derive(Clone, PartialEq, Eq)]
enum HistorySource {
    Clippy,
    User,
    System,
}

#[derive(Clone, PartialEq, Eq)]
struct HistoryItem {
    attachments: Option<Vec<DocMetadata>>,
    /// Who "wrote" this response
    source: HistorySource,
    value: String,
}

impl HistoryItem {
    pub fn as_log(&self) -> String {
        self.value.clone()
    }
}

pub struct AskClippy {
    clippy_input_ref: NodeRef,
    current_context: Option<Vec<DocMetadata>>,
    history: Vec<HistoryItem>,
    history_ref: NodeRef,
    in_progress: bool,
    status: Option<String>,
    tokens: Option<String>,
}

pub enum Msg {
    AskClippy { query: String },
    HandleAskRequest,
    HandleResponse(LLMResponsePayload),
    SetContext(Vec<DocMetadata>),
    SetError(String),
}

impl AskClippy {
    pub fn process_result(&mut self, link: Scope<AskClippy>, resp: LLMResponsePayload) {
        match &resp {
            LLMResponsePayload::LoadingModel => {
                self.in_progress = true;
                self.status = Some("Loading model...".into());
            }
            LLMResponsePayload::LoadingPrompt => {
                self.status = Some("Running inference...".into());
            }
            LLMResponsePayload::Token(token) => {
                if let Some(tokens) = self.tokens.as_mut() {
                    tokens.push_str(token);
                } else {
                    self.tokens = Some(token.to_owned());
                }
            }
            LLMResponsePayload::Finished => {
                self.in_progress = false;
            }
            LLMResponsePayload::Error(err) => {
                self.in_progress = false;
                link.send_message(Msg::SetError(err.to_owned()));
            }
        }
    }
}

impl Component for AskClippy {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();

        // Listen for new tokens
        {
            let link = link.clone();
            spawn_local(async move {
                let cb = Closure::wrap(Box::new(move |payload: JsValue| {
                    match from_value::<ListenPayload<LLMResponsePayload>>(payload) {
                        Ok(res) => link.send_message(Msg::HandleResponse(res.payload)),
                        Err(err) => {
                            console_dbg!("unable to parse LLMResult: {}", err);
                        }
                    }
                }) as Box<dyn Fn(JsValue)>);

                let _ = crate::listen(ClientEvent::LLMResponse.as_ref(), &cb).await;
                cb.forget();
            });
        }

        // Listen for events from searchbar
        {
            let link = link.clone();
            spawn_local(async move {
                let cb = Closure::wrap(Box::new(move |payload: JsValue| {
                    match from_value::<ListenPayload<SendToAskClippyPayload>>(payload) {
                        Ok(res) => {
                            if let Some(query) = res.payload.question {
                                link.send_message(Msg::AskClippy { query });
                            } else if !res.payload.docs.is_empty() {
                                link.send_message(Msg::SetContext(res.payload.docs))
                            }
                        }
                        Err(err) => {
                            console_dbg!("unable to parse SendToAskClippy: {}", err);
                        }
                    }
                }) as Box<dyn Fn(JsValue)>);

                let _ = crate::listen(ClientEvent::SendToAskClippy.as_ref(), &cb).await;
                cb.forget();
            });
        }

        Self {
            clippy_input_ref: NodeRef::default(),
            current_context: None,
            history: Vec::new(),
            history_ref: NodeRef::default(),
            in_progress: false,
            status: None,
            tokens: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::AskClippy { query } => {
                self.in_progress = true;
                // move existing result to history
                if let Some(value) = &self.tokens {
                    self.history.push(HistoryItem {
                        attachments: None,
                        source: HistorySource::Clippy,
                        value: value.to_owned(),
                    })
                }

                // capture some context to send to the model
                let mut context = self
                    .history
                    .iter()
                    // Ignore system messages
                    .filter(|x| x.source != HistorySource::System)
                    // For now, only keep the last q/a for context.
                    .rev()
                    .take(2)
                    // Map into a chat log-esque format.
                    .map(|x| ClippyContext::History(x.as_log()))
                    .collect::<Vec<_>>();
                // Reverse again so they're in order from oldest to newest.
                context.reverse();

                // push the user's question to the stack & clear the current context
                // (if any).
                self.history.push(HistoryItem {
                    attachments: self.current_context.clone(),
                    source: HistorySource::User,
                    value: query.to_string(),
                });
                // reset input
                self.current_context = None;
                self.tokens = None;
                self.status = None;

                let link = link.clone();
                spawn_local(async move {
                    if let Err(err) = tauri_invoke::<AskClippyRequest, ()>(
                        event::ClientInvoke::AskClippy,
                        AskClippyRequest {
                            question: query.to_string(),
                            context,
                        },
                    )
                    .await
                    {
                        link.send_message(Msg::SetError(err));
                    }
                });

                true
            }
            Msg::HandleAskRequest => {
                // don't submit multiple requests at a time.
                if self.in_progress {
                    return false;
                }

                if let Some(el) = self.clippy_input_ref.cast::<HtmlInputElement>() {
                    let query = el.value();
                    let query = query.trim().to_string();
                    el.set_value("");

                    if query.is_empty() {
                        return false;
                    }

                    link.send_message(Msg::AskClippy { query });
                }

                true
            }
            Msg::HandleResponse(resp) => {
                if let Some(history_el) = self.history_ref.cast::<HtmlElement>() {
                    history_el.set_scroll_top(history_el.scroll_height());
                }

                self.process_result(link.clone(), resp);
                true
            }
            Msg::SetContext(docs) => {
                self.current_context = Some(docs);
                true
            }
            Msg::SetError(err) => {
                self.status = Some(format!("Error: {err}"));
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        html! {
            <div class="flex flex-col bg-neutral-800 h-screen text-white">
                <div ref={self.history_ref.clone()} class="flex flex-col grow overflow-y-scroll place-content-end">
                    <div class="min-h-[128px] flex flex-col">
                        <HistoryLog history={self.history.clone()} />
                        { if let Some(tokens) = self.tokens.clone() {
                            html! { <HistoryLogItem source={HistorySource::Clippy} tokens={tokens} is_in_progress={self.in_progress} /> }
                        } else if let Some(status) = self.status.clone() {
                            html! { <HistoryLogItem source={HistorySource::System} tokens={status} /> }
                        } else {
                            html! {}
                        }}
                    </div>
                </div>
                <div>
                    <div class="bg-neutral-700 px-4 py-2 text-sm text-neutral-400 flex flex-row items-center gap-4">
                        <icons::Warning width="w-6" height="h-6" classes={classes!("flex-none", "text-yellow-400")} />
                        <div>
                            <span
                                class="cursor-help underline font-semibold text-cyan-500"
                                onclick={link.callback(|_| Msg::AskClippy {
                                    query: "what is a language model?".into()
                                })}
                            >
                                {"LLMs"}
                            </span>
                            {" (the tech behind this) are still experimental and responses may be inaccurate."}
                        </div>
                    </div>
                    <div class="p-4">
                        {if let Some(context) = &self.current_context {
                                html! { <AttachmentList docs={context.clone()} /> }
                        } else { html! {} }}
                        <div class="flex flex-row gap-8 items-center">
                            <textarea
                                ref={self.clippy_input_ref.clone()}
                                rows="2"
                                type="text"
                                placeholder="what is the difference between an alpaca & llama?"
                                class="text-base bg-neutral-800 text-white flex-1 outline-none active:outline-none focus:outline-none caret-white border-b-2 border-neutral-600"
                            ></textarea>
                            <btn::Btn
                                disabled={self.in_progress}
                                size={btn::BtnSize::Lg}
                                onclick={link.callback(|_| Msg::HandleAskRequest)}
                            >
                                {
                                    if self.in_progress {
                                        html! { <icons::RefreshIcon animate_spin={true} /> }
                                    } else {
                                        html! { <>{"Ask"}</> }
                                    }
                                }
                            </btn::Btn>
                        </div>
                    </div>
                </div>
            </div>
        }
    }
}

#[derive(Properties, PartialEq)]
struct AttachmentListProps {
    pub docs: Vec<DocMetadata>
}

#[function_component(AttachmentList)]
fn attachment_comp(props: &AttachmentListProps) -> Html {
    let list = props.docs.iter()
        .map(|x| html! {
            <a
                class="flex flex-row border-2 border-cyan-600 rounded-full px-4 py-1 bg-cyan-900 cursor-pointer hover:bg-cyan-600 items-center gap-2"
                href={x.open_url.clone()}
                target="_blank"
            >
                <icons::LinkIcon classes={classes!("inline")} height="h-4" width="w-4" />
                {x.title.clone()}
            </a>
        })
        .collect::<Html>();

    html! {
        <div class="mb-2 flex flex-row place-content-end items-center gap-2 text-xs">
            <span class="text-neutral-500">{"Asking about:"}</span>
            {list}
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
        .map(|item| {
            html! {
                <HistoryLogItem
                    attachments={item.attachments.clone()}
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
    pub attachments: Option<Vec<DocMetadata>>,
    pub source: HistorySource,
    pub tokens: String,
    // Is this a item currently generating tokens?
    #[prop_or_default]
    pub is_in_progress: bool,
}

#[function_component(HistoryLogItem)]
fn history_log_item(props: &HistoryLogItemProps) -> Html {
    let (user_icon, icon_pos, text_pos) = match props.source {
        HistorySource::Clippy => (html! {<>{"🤖"}</>}, None, Some("text-left")),
        HistorySource::User => (html! {<>{"🧙‍♂️"}</>}, Some("order-1"), Some("text-right")),
        HistorySource::System => (
            html! { <><img src="/icons/system-logo.png" class="h-[48px] w-[48px] rounded-full animate-pulse" /></>},
            None,
            Some("text-left"),
        ),
    };

    let html = markdown::to_html(&props.tokens.clone());
    let html = html.trim_start_matches("<p>").to_string();
    let html = html.trim_end_matches("</p>").to_string();
    let html = format!("<p class=\"inline\">{}</p>", html);

    let attachments = if let Some(docs) = &props.attachments {
        html! { <AttachmentList docs={docs.clone()} /> }
    } else {
        html! {}
    };

    html! {
        <div class="border-t border-t-neutral-700 p-4 text-sm text-white items-center flex flex-row gap-4 animate-fade-in">
            <div class={classes!("flex", "flex-none", "border", "border-cyan-600", "w-[48px]", "h-[48px]", "rounded-full", "items-center", icon_pos)}>
                <div class="text-xl mx-auto">{user_icon}</div>
            </div>
            <div class={classes!("grow", text_pos)}>
                {attachments}
                <div>
                    {Html::from_html_unchecked(AttrValue::from(html))}
                    { if props.is_in_progress {
                        html! { <div class="inline-block h-4 w-2 animate-pulse-fast bg-cyan-600 mb-[-4px]"></div> }
                    } else {
                        html! {}
                    }}
                </div>
            </div>
        </div>
    }
}
