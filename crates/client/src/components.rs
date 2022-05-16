use shared::response::{LensResult, SearchResult};
use yew::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum ResultListType {
    DocSearch,
    LensSearch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultListData {
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
                <li class={"flex bg-cyan-700 rounded-lg my-3 ml-3"}>
                    <span class={"text-4xl text-white p-3"}>{lens_name}</span>
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

    match result.result_type {
        ResultListType::DocSearch => {
            let url_link = if result.url.is_some() {
                let domain = result
                    .domain
                    .clone()
                    .unwrap_or_else(|| "example.com".to_string());
                let url = result.url.clone().unwrap();

                let path = url
                    .trim_start_matches("http://")
                    .trim_start_matches("https://")
                    .trim_start_matches(&domain);

                html! {
                    <div class={"text-sm truncate"}>
                        <a href={url.clone()} target={"_blank"} class="text-cyan-400">
                            <img
                                class="w-4 inline mr-2"
                                src={format!("https://icons.duckduckgo.com/ip3/{}.ico", domain.clone())}
                            />
                            <span class={"align-middle"}>{domain.clone()}</span>
                        </a>
                        <span class={"align-middle"}>{format!(" → {}", path)}</span>
                    </div>
                }
            } else {
                html! { <span></span> }
            };

            html! {
                <div class={vec!["text-white".into(), "p-4".into(), "border-t-2".into(), "border-neutral-600".into(), selected]}>
                    {url_link}
                    <h2 class={"text-lg truncate py-1"}>
                        {result.title.clone()}
                    </h2>
                    <div class={"text-sm leading-relaxed text-neutral-400 h-16 overflow-hidden text-ellipsis"}>
                        {result.description.clone()}
                    </div>
                </div>
            }
        }
        ResultListType::LensSearch => {
            html! {
                <div class={vec!["text-white".into(), "p-4".into(), "border-t-2".into(), "border-neutral-600".into(), selected]}>
                    <h2 class={"text-lg truncate py-1"}>
                        {result.title.clone()}
                    </h2>
                    <div class={"text-sm leading-relaxed text-neutral-400 h-16 overflow-hidden text-ellipsis"}>
                        {result.description.clone()}
                    </div>
                </div>
            }
        }
    }
}
