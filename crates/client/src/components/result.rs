use url::Url;
use yew::prelude::*;

use super::btn::DeleteButton;
use super::icons;
use shared::response::{LensResult, SearchResult};

#[derive(Properties, PartialEq)]
pub struct SearchResultProps {
    pub id: String,
    pub onclick: Callback<MouseEvent>,
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
                match url.host_str() {
                    Some("calendar.google.com") => {
                        html! { <icons::GoogleCalendar height="h-8" width="w-8" classes={classes!("m-auto")} /> }
                    }
                    // TODO: Detect file/mimetype to show even more detail icons for
                    // drive files.
                    _ => {
                        html! { <icons::GDrive height="h-8" width="w-8" classes={classes!("m-auto")} /> }
                    }
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
            }
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
                match result.domain.as_str() {
                    "calendar.google.com" => meta.push(html! { <span>{"Google Calendar"}</span> }),
                    "drive.google.com" => meta.push(html! { <span>{"Google Drive"}</span> }),
                    _ => {}
                }
            }
            "file" => {
                // Attempt to grab the folder this file resides
                let path = if let Some(segments) = url.path_segments() {
                    let mut segs = segments
                        .into_iter()
                        .map(|f| f.to_string())
                        .collect::<Vec<String>>();

                    let num_segs = segs.len();
                    if num_segs > 3 {
                        segs = segs[(num_segs - 1 - 3)..].to_vec();
                        segs.insert(0, "...".to_string());
                    }

                    segs.pop();
                    segs.join(" › ")
                } else {
                    url.path().to_string()
                };

                meta.push(html! { <span>{path}</span> });
            }
            _ => {
                meta.push(html! {
                    <span>{format!(" {}", result.domain.clone())}</span>
                });
            }
        }
    }

    // Tags
    for (tag, value) in result.tags.iter() {
        if tag == "source" || tag == "mimetype" {
            continue;
        }

        let tag_label = match tag.as_str() {
            "Lens" => "🔍",
            _ => tag,
        };

        meta.push(html! {
            <div class="text-xs flex flex-row rounded text-white bg-cyan-600 items-center">
                <div class="border-r border-cyan-900 py-0.5 px-1">
                    <small>{tag_label}</small>
                </div>
                <div class="py-0.5 px-2">
                    {value}
                </div>
            </div>
        });
    }

    let mut joined = Vec::new();
    if !meta.is_empty() {
        let last_idx = meta.len() - 1;
        for (idx, node) in meta.iter().enumerate() {
            joined.push(node.to_owned());
            if idx != last_idx {
                joined.push(html! { <span class="text-neutral-500 font-bold">{"•"}</span> });
            }
        }
    }

    html! {
        <div class="text-xs place-items-center flex flex-row gap-1.5 text-cyan-500 py-0.5 mt-1">
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
        "py-4",
        "text-white",
        "w-screen",
        "cursor-pointer",
        "hover:bg-cyan-900",
        if is_selected {
            "bg-cyan-900"
        } else {
            "bg-neutral-800"
        }
    );

    let icon = render_icon(result);
    let metadata = render_metadata(result);

    html! {
        <a id={props.id.clone()} class={component_styles} onclick={props.onclick.clone()}>
            <div class="flex flex-none pl-6 pr-2">
                <div class="flex flex-none bg-neutral-700 rounded h-12 w-12 items-center">
                    {icon}
                </div>
            </div>
            <div class="grow">
                <h2 class="text-lg truncate font-bold w-[30rem]">
                    {result.title.clone()}
                </h2>
                <div class="text-sm leading-relaxed text-neutral-400 max-h-16 overflow-hidden">
                    {result.description.clone()}
                </div>
                {metadata}
            </div>
            <div class="flex-none flex flex-col justify-self-end self-start pl-4 pr-4">
                <DeleteButton doc_id={result.doc_id.clone()} />
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
