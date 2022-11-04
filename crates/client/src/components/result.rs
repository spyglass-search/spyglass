use url::Url;
use yew::prelude::*;

use super::btn::DeleteButton;
use super::icons;
use shared::response::{LensResult, SearchResult};

#[derive(Properties, PartialEq)]
pub struct SearchResultProps {
    pub id: String,
    pub result: SearchResult,
    pub is_selected: bool,
}

fn render_icon(result: &SearchResult) -> Html {
    let url = Url::parse(&result.crawl_uri);
    let icon_size = classes!("w-8", "h-8", "m-auto");

    let icon = if let Ok(url) = &url {
        let domain = url.domain().unwrap_or("example.com").to_owned();
        match url.scheme() {
            "api" => {
                // TODO: Detect file/mimetype to show even more detailed icons
                html! {
                    <svg class={icon_size} role="img" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                        <path d="M12.01 1.485c-2.082 0-3.754.02-3.743.047.01.02 1.708 3.001 3.774 6.62l3.76 6.574h3.76c2.081 0 3.753-.02 3.742-.047-.005-.02-1.708-3.001-3.775-6.62l-3.76-6.574zm-4.76 1.73a789.828 789.861 0 0 0-3.63 6.319L0 15.868l1.89 3.298 1.885 3.297 3.62-6.335 3.618-6.33-1.88-3.287C8.1 4.704 7.255 3.22 7.25 3.214zm2.259 12.653-.203.348c-.114.198-.96 1.672-1.88 3.287a423.93 423.948 0 0 1-1.698 2.97c-.01.026 3.24.042 7.222.042h7.244l1.796-3.157c.992-1.734 1.85-3.23 1.906-3.323l.104-.167h-7.249z"/>
                    </svg>
                }
            }
            "file" => {
                if let Some((_, ext)) = result.title.rsplit_once('.') {
                    html! { <icons::FileExtIcon ext={ext.to_string()} class={icon_size} /> }
                } else {
                    html! {
                        <img class={icon_size} src={format!("https://favicon.spyglass.workers.dev/{}", domain.clone())} />
                    }
                }
            },
            _ => {
                html! {
                    <img class={icon_size} src={format!("https://favicon.spyglass.workers.dev/{}", domain.clone())} />
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

    let url = Url::parse(&result.crawl_uri);
    if let Ok(url) = url {
        match url.scheme() {
            "api" => {
                // Show friendly API name
                if result.domain == "drive.google.com" {
                    meta.push(html! { <span>{"Google Drive"}</span> });
                }
            }
            "file" => {
                // Attempt to grab the folder this file resides
                let path = if let Some(segments) = url.path_segments() {
                    let mut segs = segments
                        .into_iter()
                        .map(|f| f.to_string())
                        .collect::<Vec<String>>();
                    segs.pop();
                    segs.join("/")
                } else {
                    url.path().to_string()
                };

                meta.push(html! { <span>{path}</span> });
            }
            _ => {
                meta.push(html! {
                    <a href={result.url.clone()} target="_blank">
                        <span class="align-middle">{format!(" {}", result.domain.clone())}</span>
                    </a>
                });
            }
        }
    }

    let mut joined = Vec::new();
    if !meta.is_empty() {
        let last_idx = meta.len() - 1;
        for (idx, node) in meta.iter().enumerate() {
            joined.push(node.to_owned());
            if idx != last_idx {
                joined.push(html! { <span class="text-white font-bold">{"•"}</span> });
            }
        }
    }

    html! {
        <div class="text-xs align-middle flex flex-row gap-1 text-cyan-500">
            {joined}
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
        "items-center",
        "border-t",
        "border-neutral-600",
        "px-8",
        "py-4",
        "text-white",
        "w-screen",
        if is_selected {
            "bg-cyan-900"
        } else {
            "bg-neutral-800"
        }
    );

    let icon = render_icon(result);
    let metadata = render_metadata(result);

    html! {
        <div id={props.id.clone()} class={component_styles}>
            <div class="flex flex-none bg-neutral-700 rounded h-12 w-12 items-center">{icon}</div>
            <div class="grow">
                <h2 class="text-lg truncate font-bold w-96">
                    <a href={result.url.clone()}>{result.title.clone()}</a>
                </h2>
                {metadata}
                <div class="text-sm leading-relaxed text-neutral-400">
                    {result.description.clone()}
                </div>
            </div>
            <div class="flex-none flex flex-col justify-self-end self-start">
                <DeleteButton doc_id={result.doc_id.clone()} />
            </div>
        </div>
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
        "border-t",
        "border-neutral-600",
        "px-8",
        "py-4",
        "text-white",
        if is_selected {
            "bg-cyan-900"
        } else {
            "bg-neutral-800"
        }
    );

    html! {
        <div id={props.id.clone()} class={component_styles}>
            <h2 class="text-2xl truncate py-1">
                {result.title.clone()}
            </h2>
            <div class="text-sm leading-relaxed text-neutral-400">
                {result.description.clone()}
            </div>
        </div>
    }
}
