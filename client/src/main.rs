use serde::{Deserialize, Serialize};
use std::cmp::PartialEq;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlInputElement};
use yew::prelude::*;

#[wasm_bindgen(module = "/public/glue.js")]
extern "C" {
    #[wasm_bindgen(js_name = invokeSearch, catch)]
    pub async fn run_search(query: String) -> Result<JsValue, JsValue>;
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
    let search_results = use_state_eq(|| Vec::new());
    let query = use_state_eq(|| "".to_string());

    {
        let search_results = search_results.clone();
        use_effect_with_deps(
            move |query| {
                update_results(search_results, query.clone());
                || ()
            },
            (*query).clone(),
        );
    }

    let results = search_results
        .iter()
        .map(|res| {
            html! {
                <div class={"result-item"}>
                    <div class={"result-url"}>
                        <a href={res.url.clone()}>{format!("{}", res.url.clone())}</a>
                    </div>
                    <h2 class={"result-title"}>{res.title.clone()}</h2>
                    <div class={"result-description"}>{res.description.clone()}</div>
                </div>
            }
        })
        .collect::<Html>();

    let onkeyup = {
        let query = query.clone();
        Callback::from(move |e: KeyboardEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            query.set(input.value());
        })
    };

    html! {
        <div>
            <input
                type={"text"}
                class={"search-box"}
                placeholder={"Spyglass Search"}
                value={(*query).clone()}
                {onkeyup}
                spellcheck={"false"}
            />
            <div class={"search-results-list"}>
                { results }
            </div>
        </div>
    }
}

fn update_results(handle: UseStateHandle<Vec<SearchResult>>, query: String) {
    if query.len() < 2 {
        return;
    }

    spawn_local(async move {
        match run_search(query).await {
            Ok(results) => {
                let results: Vec<SearchResult> = results.into_serde().unwrap();
                handle.set(results);
            }
            Err(e) => {
                let window = window().unwrap();
                window
                    .alert_with_message(&format!("Error: {:?}", e))
                    .unwrap();
            }
        }
    })
}
