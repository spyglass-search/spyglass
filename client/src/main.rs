use std::cmp::PartialEq;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use yew::prelude::*;
use serde::{Deserialize, Serialize};

#[wasm_bindgen(module = "/public/glue.js")]
extern "C" {
    #[wasm_bindgen(js_name = invokeSearch, catch)]
    pub async fn search(query: String) -> Result<JsValue, JsValue>;
}

fn main() {
    yew::start_app::<App>();
}


#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct SearchResult {
    title: String,
    description: String,
    url: String,
}

#[function_component(App)]
pub fn app() -> Html {
    let search = use_state_eq(|| Vec::new());
    let query = use_state_eq(|| "query".to_string());

    {
        let search = search.clone();
        use_effect_with_deps(
            move |query| {
                update_results(search, query.clone());
                || ()
            },
            (*query).clone(),
        );
    }

    let results = search.iter().map(|res| html! {
        <p>{res.title.clone()}</p>
    }).collect::<Html>();

    html! {
        <div>
            <h2 class={"heading"}>{"Hello, World!"}</h2>
            <h3>{"Results"}</h3>
            { results }
        </div>
    }
}

fn update_results(handle: UseStateHandle<Vec<SearchResult>>, query: String) {
    spawn_local(async move {
        match search(query).await {
            Ok(results) => {
                let results: Vec<SearchResult> = results.into_serde().unwrap();
                handle.set(results);
            },
            Err(e) => {
                let window = window().unwrap();
                window.alert_with_message(&format!("Error: {:?}", e))
                    .unwrap();
            }
        }
    })
}