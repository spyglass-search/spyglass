use serde::{Deserialize, Serialize};
use yew::prelude::*;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct SearchResult {
    title: String,
    description: String,
    url: String,
}

pub fn search_result_component(res: &SearchResult) -> Html {
    html! {
        <div class={"result-item"}>
            <div class={"result-url"}>
                <a href={res.url.clone()} target={"_blank"}>
                    {res.url.clone()}
                </a>
            </div>
            <h2 class={"result-title"}>{res.title.clone()}</h2>
            <div class={"result-description"}>{res.description.clone()}</div>
        </div>
    }
}
