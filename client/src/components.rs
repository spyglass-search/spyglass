use serde::{Deserialize, Serialize};
use yew::prelude::*;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct SearchResult {
    title: String,
    description: String,
    url: String,
}

pub fn search_result_component(res: &SearchResult, is_selected: bool) -> Html {
    let mut selected: Option<String> = None;
    if is_selected {
        selected = Some("result-selected".to_string());
    }

    html! {
        <div class={vec![Some("result-item".to_string()), selected]}>
            <div class={"result-url"}>
                <a href={res.url.clone()} target={"_blank"}>
                    {format!("🌐 {}", res.url.clone())}
                </a>
            </div>
            <h2 class={"result-title"}>{res.title.clone()}</h2>
            <div class={"result-description"}>{res.description.clone()}</div>
        </div>
    }
}
