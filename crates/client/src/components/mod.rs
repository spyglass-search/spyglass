pub mod btn;
pub mod icons;
pub mod lens;

use yew::prelude::*;

use btn::DeleteButton;
use shared::response::{LensResult, SearchResult};
use url::Url;

#[derive(Clone, Debug, PartialEq)]
pub enum ResultListType {
    DocSearch,
    LensSearch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultListData {
    pub id: String,
    pub domain: Option<String>,
    pub title: String,
    pub description: String,
    pub url: Option<String>,
    pub score: f32,
    pub result_type: ResultListType,
}

impl From<&LensResult> for ResultListData {
    fn from(x: &LensResult) -> Self {
        ResultListData {
            id: x.title.clone(),
            description: x.description.clone(),
            domain: None,
            result_type: ResultListType::LensSearch,
            score: 1.0,
            title: x.title.clone(),
            url: None,
        }
    }
}

impl From<&SearchResult> for ResultListData {
    fn from(x: &SearchResult) -> Self {
        ResultListData {
            id: x.doc_id.clone(),
            description: x.description.clone(),
            domain: Some(x.domain.clone()),
            result_type: ResultListType::DocSearch,
            score: x.score,
            title: x.title.clone(),
            url: Some(x.url.clone()),
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct SelectLensProps {
    pub lens: Vec<String>,
}

/// Render a list of selected lenses
#[function_component(SelectedLens)]
pub fn selected_lens_list(props: &SelectLensProps) -> Html {
    let items = props
        .lens
        .iter()
        .map(|lens_name: &String| {
            html! {
                <li class="flex bg-cyan-700 rounded-lg my-3 ml-3">
                    <span class="text-4xl text-white p-3">{lens_name}</span>
                </li>
            }
        })
        .collect::<Html>();

    html! {
        <ul class="flex bg-neutral-800">
            {items}
        </ul>
    }
}

#[derive(Properties, PartialEq)]
pub struct SearchResultProps {
    pub result: ResultListData,
    pub is_selected: bool,
}

/// Render search results
#[function_component(SearchResultItem)]
pub fn search_result_component(props: &SearchResultProps) -> Html {
    let is_selected = props.is_selected;
    let result = &props.result;

    let mut selected: String = "bg-neutral-800".into();
    if is_selected {
        selected = "bg-cyan-900".into();
    }

    let component_styles = vec![
        "border-t".into(),
        "border-neutral-600".into(),
        "p-4".into(),
        "pr-0".into(),
        "text-white".into(),
        selected,
    ];

    match result.result_type {
        ResultListType::DocSearch => {
            let url_link = if let Some(url) = &result.url {
                if let Ok(url) = Url::parse(url) {
                    let domain = result
                        .domain
                        .clone()
                        .unwrap_or_else(|| "example.com".to_string());

                    let path = url.path();

                    let uri_icon = if url.scheme() == "file" {
                        html! {
                            <icons::DesktopComputerIcon
                                classes={classes!("inline", "align-middle")}
                            />
                        }
                    } else {
                        html! {
                            <img class="w-3 inline align-middle"
                                src={format!("https://icons.duckduckgo.com/ip3/{}.ico", domain.clone())}
                            />
                        }
                    };

                    html! {
                        <div class="text-xs truncate">
                            <a href={url.clone().to_string()} target="_blank">
                                {uri_icon}
                                {
                                    if url.scheme() == "file" {
                                        html!{}
                                    } else {
                                        html!{ <span class="align-middle text-cyan-400">{format!(" {}", domain.clone())}</span> }
                                    }
                                }
                                <span class="align-middle">{format!(" → {}", path)}</span>
                            </a>
                        </div>
                    }
                } else {
                    html! { <span></span> }
                }
            } else {
                html! { <span></span> }
            };

            html! {
                <div class={component_styles}>
                    <div class="float-right pl-4 mr-2 h-28">
                        <DeleteButton doc_id={result.id.clone()} />
                    </div>
                    {url_link}
                    <h2 class="text-lg truncate py-1">
                        {result.title.clone()}
                    </h2>
                    <div class="text-sm leading-relaxed text-neutral-400 h-16 overflow-hidden text-ellipsis">
                        {result.description.clone()}
                    </div>
                </div>
            }
        }
        ResultListType::LensSearch => {
            html! {
                <div class={component_styles}>
                    <h2 class="text-2xl truncate py-1">
                        {result.title.clone()}
                    </h2>
                    <div class="text-sm leading-relaxed text-neutral-400 h-16 overflow-hidden text-ellipsis">
                        {result.description.clone()}
                    </div>
                </div>
            }
        }
    }
}

#[derive(PartialEq, Properties)]
pub struct HeaderProps {
    pub label: String,
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub classes: Classes,
    #[prop_or_default]
    pub tabs: Html,
}

#[function_component(Header)]
pub fn header(props: &HeaderProps) -> Html {
    html! {
        <div class={classes!(props.classes.clone(), "pt-4", "px-8", "top-0", "sticky", "bg-stone-800", "z-400", "border-b-2", "border-stone-900")}>
            <div class="flex flex-row items-center gap-4 pb-4">
                <h1 class="text-2xl grow">{props.label.clone()}</h1>
                {props.children.clone()}
            </div>
            <div class="flex flex-row items-center gap-4">
                {props.tabs.clone()}
            </div>
        </div>
    }
}

#[derive(Debug)]
pub struct TabEvent {
    pub tab_idx: usize,
    pub tab_name: String,
}

#[derive(PartialEq, Properties)]
pub struct TabsProps {
    #[prop_or_default]
    pub onchange: Callback<TabEvent>,
    pub tabs: Vec<String>,
}

#[function_component(Tabs)]
pub fn tabs(props: &TabsProps) -> Html {
    let active_idx = use_state_eq(|| 0);
    let tab_styles = classes!(
        "block",
        "border-b-2",
        "px-4",
        "py-2",
        "text-xs",
        "font-medium",
        "uppercase",
        "hover:bg-stone-700",
        "hover:border-green-500",
    );

    let onchange = props.onchange.clone();
    let tabs = props.tabs.clone();
    use_effect_with_deps(
        move |updated| {
            onchange.emit(TabEvent {
                tab_idx: **updated,
                tab_name: tabs[**updated].clone(),
            });
            || {}
        },
        active_idx.clone(),
    );

    html! {
        <ul class="flex flex-row list-none gap-4">
        {
            props.tabs.iter().enumerate().map(|(idx, tab_name)| {
                let border = if idx == *active_idx { "border-green-500" } else { "border-transparent" };
                let active_idx = active_idx.clone();
                let onclick = Callback::from(move |_| {
                    active_idx.set(idx);
                });

                html! {
                    <li>
                        <button
                            onclick={onclick}
                            class={classes!(tab_styles.clone(), border)}
                        >
                            {tab_name}
                        </button>
                    </li>
                }
            })
            .collect::<Html>()
        }
        </ul>
    }
}
