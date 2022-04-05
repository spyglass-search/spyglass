use shared::response::{LensResult, SearchResult};
use yew::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum ResultListType {
    DocSearch,
    LensSearch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultListData {
    pub title: String,
    pub description: String,
    pub url: Option<String>,
    pub score: f32,
    pub result_type: ResultListType,
}

impl From<&LensResult> for ResultListData {
    fn from(x: &LensResult) -> Self {
        ResultListData {
            title: x.title.clone(),
            description: x.description.clone(),
            url: None,
            score: 1.0,
            result_type: ResultListType::LensSearch,
        }
    }
}

impl From<&SearchResult> for ResultListData {
    fn from(x: &SearchResult) -> Self {
        ResultListData {
            title: x.title.clone(),
            description: x.description.clone(),
            url: Some(x.url.clone()),
            score: x.score,
            result_type: ResultListType::DocSearch,
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
                let url = res.url.clone();
                html! {
                    <div class={"result-url"}>
                        <a href={res.url.clone()} target={"_blank"}>
                            {format!("🌐 {}", url.unwrap())}
                        </a>
                    </div>
                }
            } else {
                html! { <span></span> }
            };

            html! {
                <div class={vec![Some("result-item".to_string()), selected]}>
                    <div class={"result-url"}>
                        {url_link}
                    </div>
                    <h2 class={"result-title"}>{res.title.clone()}</h2>
                    <div class={"result-description"}>{res.description.clone()}</div>
                    <div class={"result-score"}>{res.score}</div>
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
