use js_sys::decode_uri_component;
use shared::response::{LensResult, SearchResult};
use url::Url;
use yew::prelude::*;

use super::icons;
use super::tag::{Tag, TagIcon};

#[derive(Properties, PartialEq)]
pub struct SearchResultProps {
    pub id: String,
    #[prop_or_default]
    pub onclick: Callback<MouseEvent>,
    pub result: SearchResult,
    #[prop_or_default]
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

    let icon_classes = classes!("mt-1", "flex", "flex-none", "pr-2", "pl-6");
    let title_classes = classes!("text-base", "truncate", "font-semibold", "w-[30rem]");

    html! {
        <a id={props.id.clone()} class={component_styles} onclick={props.onclick.clone()}>
            <div class={icon_classes}>
                <div class="relative flex-none bg-neutral-700 rounded h-12 w-12 items-center">
                    {icon}
                </div>
            </div>
            <div class="grow">
                <div class="text-xs text-cyan-500">{domain}</div>
                <h2 class={title_classes}>{title}</h2>
                <div class="text-sm leading-relaxed text-neutral-400 max-h-10 overflow-hidden">
                    {Html::from_html_unchecked(result.description.clone().into())}
                </div>
                {metadata}
                <div class="text-neutral-600 text-xs pt-1">{result.score}</div>
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

#[function_component(WebSearchResultItem)]
pub fn web_search_result_component(props: &SearchResultProps) -> Html {
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

    let metadata = render_metadata(result);

    let score = {
        #[cfg(debug_assertions)]
        html! { <div class="text-neutral-600 text-xs pt-1">{result.score}</div> }

        #[cfg(not(debug_assertions))]
        html! {}
    };


    html! {
        <a
            id={props.id.clone()}
            href={props.result.url.clone()}
            class={component_styles}
            target="_blank"
        >
            <div class={classes!("mt-1", "flex", "flex-none")}>
                <div class="relative flex-none bg-neutral-700 rounded-sm h-6 w-6 items-center">
                    <img class="w-4 h-4 m-auto mt-1"
                        alt="website icon"
                        src={format!("https://favicon.spyglass.workers.dev/{}", result.domain.clone())}
                    />
                </div>
            </div>
            <div class="grow">
                <div class="text-xs text-cyan-500">
                    <span>{format!("{}", result.domain.clone())}</span>
                </div>
                <h2 class={classes!("text-base", "font-semibold")}>{result.title.clone()}</h2>
                <div class="text-sm leading-relaxed text-neutral-400">
                    {Html::from_html_unchecked(result.description.clone().into())}
                </div>
                {metadata}
                {score}
            </div>
        </a>
    }
}

#[derive(Properties, PartialEq)]
pub struct ResultPaginatorProps {
    pub children: Children,
    pub page_size: usize,
}

#[function_component(ResultPaginator)]
pub fn result_paginator(props: &ResultPaginatorProps) -> Html {
    let page: UseStateHandle<usize> = use_state(|| 0);

    let num_pages = props.children.len() / props.page_size;

    let result_html = props
        .children
        .iter()
        .skip(*page * props.page_size)
        .take(props.page_size)
        .collect::<Vec<Html>>();

    let mut pages_html = Vec::new();
    let component_classes = classes!(
        "cursor-pointer",
        "relative",
        "block",
        "rounded",
        "px-3",
        "py-1.5",
        "text-sm",
        "text-neutral-600",
        "transition-all",
        "duration-300",
        "hover:bg-neutral-100",
        "dark:text-white",
        "dark:hover:bg-neutral-700",
        "dark:hover:text-white"
    );

    for page_num in 0..num_pages {
        // Highlight the current page
        let mut classes = component_classes.clone();
        if *page == page_num {
            classes.push("bg-cyan-500");
        }

        let page_handle = page.clone();
        pages_html.push(html! {
            <li>
                <a
                    class={classes}
                    onclick={move |_| page_handle.set(page_num)}
                >
                    {page_num + 1}
                </a>
            </li>
        });
    }

    let page_handle = page.clone();
    let handle_previous = move |_| {
        if *page_handle > 0 {
            page_handle.set(*page_handle - 1);
        }
    };

    let page_handle = page.clone();
    let handle_next = move |_| {
        if *page_handle < (num_pages - 1) {
            page_handle.set(*page_handle + 1);
        }
    };

    html! {
        <div>
            <div>{result_html}</div>
            <nav class="border-t-neutral-700 border-t py-4 mt-4">
                <ul class="list-style-none flex flex-row gap-2 justify-around">
                    <li>
                        <button onclick={handle_previous} class={component_classes.clone()} disabled={*page == 0}>{"Previous"}</button>
                    </li>
                    {pages_html}
                    <li>
                        <button onclick={handle_next} class={component_classes} disabled={*page == num_pages}>{"Next"}</button>
                    </li>
                </ul>
            </nav>
        </div>
    }
}
