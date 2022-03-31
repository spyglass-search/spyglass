use serde::{Deserialize, Serialize};
use yew::prelude::*;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct SearchResult {
    pub title: String,
    pub description: String,
    pub url: Option<String>,
    pub score: f32,
}

pub fn lens_list(lens: &[String]) -> Html {
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

pub fn search_result_component(res: &SearchResult, is_selected: bool) -> Html {
    let mut selected: Option<String> = None;
    if is_selected {
        selected = Some("result-selected".to_string());
    }

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
