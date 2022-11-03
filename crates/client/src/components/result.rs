use url::Url;
use yew::prelude::*;

use super::btn::DeleteButton;
use super::icons;
use super::{ResultListData, ResultListType};

#[derive(Properties, PartialEq)]
pub struct SearchResultProps {
    pub id: String,
    pub result: ResultListData,
    pub is_selected: bool,
}

fn render_icon(result: &ResultListData) -> Html {
    let url = result.url.clone().and_then(|s| Url::parse(&s).ok());
    let icon_size = classes!("w-8", "h-8", "m-auto");

    let icon = if let Some(url) = &url {
        let domain = url.domain().unwrap_or("example.com");
        if url.scheme() == "file" {
            html! {
                <icons::DesktopComputerIcon classes="m-auto" height="h-8" width="w-8" />
            }
        } else {
            html! {
                <img class={icon_size} src={format!("https://favicon.spyglass.workers.dev/{}", domain.clone())} />
            }
        }
    } else {
        html! {}
    };

    icon
}

fn render_metadata(result: &ResultListData) -> Html {
    let mut meta = Vec::new();

    let url = result.url.clone().and_then(|s| Url::parse(&s).ok());
    if let (Some(url), Some(domain)) = (url, &result.domain) {
        if url.scheme() == "file" {
            // Attempt to grab the folder this file resides
            let path = if let Some(segments) = url.path_segments() {
                let mut segs = segments.into_iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<String>>();
                segs.pop();
                segs.join("/")
            } else {
                url.path().to_string()
            };

            meta.push(html! { <span>{path}</span> });
        } else {
            meta.push(
                html! {
                    <a href={result.url.clone()} target="_blank">
                        <span class="align-middle">{format!(" {}", domain.clone())}</span>
                    </a>
                }
            );
        }
    }

    let mut joined = Vec::new();
    if meta.len() > 0 {
        let last_idx = meta.len() - 1;
        for (idx, node) in meta.iter().enumerate() {
            joined.push(node.to_owned());
            if idx != last_idx {
                joined.push(html!{ <span class="text-white font-bold">{"•"}</span> });
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
        "px-8", "py-4",
        "text-white",
        if is_selected {
            "bg-cyan-900"
        } else {
            "bg-neutral-800"
        }
    );

    match result.result_type {
        ResultListType::DocSearch => {
            let icon = render_icon(result);
            let metadata = render_metadata(result);

            html! {
                <div id={props.id.clone()} class={component_styles}>
                    <div class="flex flex-none bg-neutral-700 rounded h-12 w-12 items-center">{icon}</div>
                    <div class="grow">
                        <h2 class="text-lg truncate font-bold">
                            {result.title.clone()}
                        </h2>
                        {metadata}
                        <div class="text-sm leading-relaxed text-neutral-400">
                            {result.description.clone()}
                        </div>
                    </div>
                    <div class="shrink flex flex-col justify-self-end self-start">
                        <DeleteButton doc_id={result.id.clone()} />
                    </div>
                </div>
            }
        }
        ResultListType::LensSearch => {
            html! {
                <div id={props.id.clone()} class={component_styles}>
                    <h2 class="text-2xl truncate py-1">
                        {result.title.clone()}
                    </h2>
                    <div class="text-sm leading-relaxed text-neutral-400 h-12 overflow-hidden text-ellipsis">
                        {result.description.clone()}
                    </div>
                </div>
            }
        }
    }
}
