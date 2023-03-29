use gloo::console::console_dbg;
use js_sys::decode_uri_component;
use serde_wasm_bindgen::from_value;
use shared::event::{LLMResultPayload, ListenPayload};
use shared::request::AskClippyRequest;
use shared::{constants::FEEDBACK_FORM, event};
use shared::{
    event::ClientEvent,
    response::{LensResult, SearchResult},
};
use url::Url;
use wasm_bindgen::{prelude::Closure, JsValue};
use yew::{platform::spawn_local, prelude::*};
use yew_router::Routable;

use super::{
    btn, icons,
    tag::{Tag, TagIcon},
};
use crate::{pages::Tab, tauri_invoke, Route};

#[derive(Properties, PartialEq)]
pub struct SearchResultProps {
    pub id: String,
    pub onclick: Callback<MouseEvent>,
    pub result: SearchResult,
    pub is_selected: bool,
}

fn render_icon(result: &SearchResult) -> Html {
    let url = Url::parse(&result.crawl_uri);
    let icon_size = classes!("w-8", "h-8", "m-auto", "mt-2");

    let is_directory = result.tags.iter().any(|(label, value)| {
        label.to_lowercase() == "type" && value.to_lowercase() == "directory"
    });

    let is_file = result
        .tags
        .iter()
        .any(|(label, value)| label.to_lowercase() == "type" && value.to_lowercase() == "file");

    let ext = if let Some((_, ext)) = result.title.rsplit_once('.') {
        ext.to_string()
    } else {
        "txt".to_string()
    };

    let icon = if let Ok(url) = &url {
        let domain = url.domain().unwrap_or("example.com").to_owned();
        match url.scheme() {
            "api" => {
                let connection = url.host_str().unwrap_or_default();
                if is_directory {
                    html! {
                        <>
                            <icons::FolderIcon height="h-8" width="w-8" classes="m-auto mt-2" />
                            <div class="absolute bg-cyan-500 bottom-0 right-0 w-5 h-5 p-0.5 rounded">
                                {icons::connection_icon(connection, "h-4", "w-4", classes!())}
                            </div>
                        </>
                    }
                } else if is_file {
                    html! {
                        <>
                            <icons::FileExtIcon {ext} class={icon_size} />
                            <div class="absolute bg-cyan-500 bottom-0 right-0 w-5 h-5 p-0.5 rounded">
                                {icons::connection_icon(connection, "h-4", "w-4", classes!())}
                            </div>
                        </>
                    }
                } else {
                    icons::connection_icon(connection, "h-8", "w-8", classes!("m-auto", "mt-2"))
                }
            }
            "file" => {
                let is_directory = result.tags.iter().any(|(label, value)| {
                    label.to_lowercase() == "type" && value.to_lowercase() == "directory"
                });

                if is_directory {
                    html! { <icons::FolderIcon height="h-8" width="w-8" classes="bg-color-white m-auto mt-2" /> }
                } else {
                    html! { <icons::FileExtIcon {ext} class={icon_size} /> }
                }
            }
            _ => {
                html! {
                    <img class={icon_size} alt="Website" src={format!("https://favicon.spyglass.workers.dev/{}", domain.clone())} />
                }
            }
        }
    } else {
        html! {}
    };

    icon
}

// TODO: Pull this special metadata from tags provided by the backend.
fn render_metadata(result: &SearchResult) -> Html {
    let mut meta = Vec::new();
    // Generate the icons/labels required for tags
    let mut priority_tags = Vec::new();
    let mut normal_tags = Vec::new();

    let result_type = result
        .tags
        .iter()
        .find(|(label, _)| label.to_lowercase() == "type")
        .map(|(_, val)| val.as_str())
        .unwrap_or_default();

    for (tag, value) in result.tags.iter() {
        let tag = tag.to_lowercase();
        if tag == "source" || tag == "mimetype" {
            continue;
        }

        if result_type == "repository" && tag == "repository" {
            continue;
        }

        if tag == "favorited" {
            priority_tags.push(html! { <TagIcon label={tag} value={value.clone()} /> });
        } else {
            normal_tags.push(html! { <Tag label={tag} value={value.clone()} /> });
        }
    }

    meta.extend(priority_tags);
    meta.extend(normal_tags);

    html! {
        <div class="text-xs place-items-center flex flex-row flex-wrap gap-2 text-cyan-500 py-0.5 mt-1.5">
            {meta}
        </div>
    }
}

/// Render search results
#[function_component(SearchResultItem)]
pub fn search_result_component(props: &SearchResultProps) -> Html {
    let is_selected = props.is_selected;
    let result = &props.result;

    let component_styles = classes!(
        "flex",
        "flex-row",
        "gap-4",
        "rounded",
        "py-2",
        "pr-2",
        "mt-2",
        "text-white",
        "cursor-pointer",
        "active:bg-cyan-900",
        "scroll-mt-2",
        if is_selected {
            "bg-cyan-900"
        } else {
            "bg-neutral-800"
        }
    );

    let icon = render_icon(result);
    let metadata = render_metadata(result);

    let mut title = result.title.clone();
    if result.url.starts_with("file://") {
        if let Ok(url) = Url::parse(&result.url) {
            if let Some(path) = shorten_file_path(&url, 3, true) {
                title = path;
            }
        }
    }

    let url = Url::parse(&result.crawl_uri);

    let domain = if let Ok(url) = url {
        if let Some(path) = shorten_file_path(&url, 3, false) {
            html! { <span>{path}</span> }
        } else {
            html! {
            <span>{format!(" {}", result.domain.clone())}</span>
            }
        }
    } else {
        html! {}
    };

    html! {
        <a id={props.id.clone()} class={component_styles} onclick={props.onclick.clone()}>
            <div class="mt-1 flex flex-none pl-6 pr-2">
                <div class="relative flex-none bg-neutral-700 rounded h-12 w-12 items-center">
                    {icon}
                </div>
            </div>
            <div class="grow">
                <div class="text-xs text-cyan-500">{domain}</div>
                <h2 class="text-base truncate font-semibold w-[30rem]">
                    {title}
                </h2>
                <div class="text-sm leading-relaxed text-neutral-400 max-h-10 overflow-hidden">
                    {Html::from_html_unchecked(result.description.clone().into())}
                </div>
                {metadata}
            </div>
        </a>
    }
}

#[derive(Properties, PartialEq, Eq)]
pub struct LensResultProps {
    pub id: String,
    pub result: LensResult,
    pub is_selected: bool,
}

#[function_component(LensResultItem)]
pub fn lens_result_component(props: &LensResultProps) -> Html {
    let is_selected = props.is_selected;
    let result = &props.result;

    let component_styles = classes!(
        "flex",
        "flex-col",
        "p-2",
        "mt-2",
        "text-white",
        "rounded",
        "scroll-mt-2",
        if is_selected {
            "bg-cyan-900"
        } else {
            "bg-neutral-800"
        }
    );

    html! {
        <div id={props.id.clone()} class={component_styles}>
            <h2 class="text-2xl truncate py-1">
                {result.label.clone()}
            </h2>
            <div class="text-sm leading-relaxed text-neutral-400">
                {result.description.clone()}
            </div>
        </div>
    }
}

fn shorten_file_path(url: &Url, max_segments: usize, show_file_name: bool) -> Option<String> {
    if url.scheme() == "file" {
        // Attempt to grab the folder this file resides
        let path = if let Some(segments) = url.path_segments() {
            let mut segs = segments
                .into_iter()
                .filter_map(|f| {
                    if f.is_empty() {
                        None
                    } else {
                        decode_uri_component(f)
                            .map(|s| s.as_string())
                            .unwrap_or_else(|_| Some(f.to_string()))
                    }
                })
                .collect::<Vec<String>>();

            if !show_file_name {
                segs.pop();
            }

            let num_segs = segs.len();
            if num_segs > max_segments {
                segs = segs[(num_segs - max_segments)..].to_vec();
                segs.insert(0, "...".to_string());
            }

            segs.join(" › ")
        } else {
            let path_str = url.path().to_string();
            decode_uri_component(&path_str)
                .map(|s| s.as_string())
                .unwrap_or_else(|_| Some(path_str.to_string()))
                .unwrap_or_else(|| path_str.to_string())
        };

        return Some(path);
    }

    None
}

#[derive(Properties, PartialEq)]
pub struct FeedbackProps {
    pub query: String,
    pub num_docs: u32,
}

#[function_component(FeedbackResult)]
pub fn feedback_result(props: &FeedbackProps) -> Html {
    html! {
      <div class="p-4 text-base">
        <div class="text-xl text-white">{"No results found 😭"}</div>
        {if props.num_docs > 0 {
            html! {
                <div class="text-neutral-400 flex flex-row gap-2 items-center">
                    {"Help us improve our results."}
                    <btn::Btn href={FEEDBACK_FORM} size={btn::BtnSize::Xs}>
                        {"Click to send feedback"}
                    </btn::Btn>
                </div>
            }
        } else {
            let discover_cb = Callback::from(move |_| {
                spawn_local(async move {
                    let route = Route::SettingsPage { tab: Tab::Discover };
                    let _ = tauri_invoke::<event::NavigateParams, ()>(
                        event::ClientInvoke::Navigate,
                        event::NavigateParams { page: route.to_path() }
                    ).await;
                });
            });

            let add_cb = Callback::from(move |_| {
                spawn_local(async move {
                    let route = Route::SettingsPage { tab: Tab::ConnectionsManager };
                    let _ = tauri_invoke::<event::NavigateParams, ()>(
                        event::ClientInvoke::Navigate,
                        event::NavigateParams { page: route.to_path() }
                    ).await;
                });
            });

            html! {
                <div class="text-neutral-400 flex flex-row gap-2 items-center">
                    {"You don't currently have any documents in your library."}
                    <btn::Btn onclick={discover_cb} size={btn::BtnSize::Xs}>
                        {"Discover Lenses"}
                    </btn::Btn>
                    <btn::Btn onclick={add_cb} size={btn::BtnSize::Xs}>
                        {"Add Connection"}
                    </btn::Btn>
                </div>
            }
        }}
      </div>
    }
}

pub struct LLMResult {
    clippy_input_ref: NodeRef,
    tokens: String,
    listeners: Vec<JsValue>,
    in_progress: bool,
}

pub enum LLMResultMsg {
    AddListener(JsValue),
    AskClippy,
    UpdateTokens(String),
}

impl Component for LLMResult {
    type Message = LLMResultMsg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();

        // Listen for new tokens
        {
            let link = link.clone();
            spawn_local(async move {
                let link_cb = link.clone();
                let cb = Closure::wrap(Box::new(move |payload: JsValue| {
                    if let Ok(res) = from_value::<ListenPayload<LLMResultPayload>>(payload) {
                        link_cb.send_message(LLMResultMsg::UpdateTokens(res.payload.token));
                    }
                }) as Box<dyn Fn(JsValue)>);

                if let Ok(listener) = crate::listen(ClientEvent::LLMResponse.as_ref(), &cb).await {
                    link.send_message(LLMResultMsg::AddListener(listener));
                }

                cb.forget();
            });
        }

        Self {
            clippy_input_ref: NodeRef::default(),
            tokens: String::new(),
            listeners: Vec::new(),
            in_progress: false,
        }
    }

    fn destroy(&mut self, _ctx: &Context<Self>) {
        // for listener in self.listeners {
        //     let unlisten: Result<dyn Fn()> = listener.try_into();
        // }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            LLMResultMsg::AddListener(listener) => {
                self.listeners.push(listener);
                false
            }
            LLMResultMsg::AskClippy => {
                self.in_progress = true;
                spawn_local(async move {
                    let res = tauri_invoke::<AskClippyRequest, ()>(
                        event::ClientInvoke::AskClippy,
                        AskClippyRequest {
                            question: "what is an alpaca?".into(),
                            doc_ids: [].into(),
                        },
                    )
                    .await;
                    console_dbg!(res);
                });
                true
            }
            LLMResultMsg::UpdateTokens(token) => {
                self.tokens += &token;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        html! {
            <div class="p-4">
                <div class="flex flex-row gap-1">
                    <input
                        ref={self.clippy_input_ref.clone()}
                        type="text"
                        placeholder="ask clippy"
                        class="bg-neutral-800 text-white text-2xl py-3 overflow-hidden flex-1 outline-none active:outline-none focus:outline-none caret-white"
                    />
                    <btn::Btn
                        disabled={self.in_progress}
                        size={btn::BtnSize::Lg}
                        onclick={link.callback(|_| LLMResultMsg::AskClippy)}
                    >
                        { if self.in_progress { "..." } else { "Ask"} }
                    </btn::Btn>
                </div>
                <div>
                    { if !self.tokens.is_empty() {
                        html! { <div>{self.tokens.clone()}</div> }
                    } else { html! {} }}
                </div>
            </div>
        }
    }
}
