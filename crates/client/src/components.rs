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

/// Render a list of selected lenses
pub fn selected_lens_list(lens: &[String]) -> Html {
    let items = lens
        .iter()
        .map(|lens_name: &String| {
            html! {
                <li class={"lens"}>
                    <span class={"lens-title"}>{lens_name}</span>
                </li>
            }
        })
        .collect::<Html>();

    html! {
        <ul class={"lenses"}>
            {items}
        </ul>
    }
}

/// Render search results
pub fn search_result_component(res: &ResultListData, is_selected: bool) -> Html {
    let mut selected: Option<String> = None;
    if is_selected {
        selected = Some("result-selected".to_string());
    }

    match res.result_type {
        ResultListType::DocSearch => {
            let url_link = if res.url.is_some() {
                let domain = res
                    .domain
                    .clone()
                    .unwrap_or_else(|| "example.com".to_string());
                let url = res.url.clone().unwrap();

                let path = url
                    .trim_start_matches("http://")
                    .trim_start_matches("https://")
                    .trim_start_matches(&domain);

                html! {
                    <div class={"result-url"}>
                        <a href={url.clone()} target={"_blank"}>
                            <img src={format!("https://icons.duckduckgo.com/ip3/{}.ico", domain.clone())} />
                            {domain.clone()}
                        </a>
                        <span>{format!(" → {}", path)}</span>
                    </div>
                }
            } else {
                html! { <span></span> }
            };

            html! {
                <div class={vec![Some("result-item".to_string()), selected]}>
                    {url_link}
                    <h2 class={"result-title"}>{res.title.clone()}</h2>
                    <div class={"result-description"}>{res.description.clone()}</div>
                </div>
            }
        }
        ResultListType::LensSearch => {
            html! {
                <div class={vec![Some("lens-result-item".to_string()), selected]}>
                    <h2 class={"result-title"}>{res.title.clone()}</h2>
                    <div class={"result-description"}>{res.description.clone()}</div>
                </div>
            }
        }
    }
}
