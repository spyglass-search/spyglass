use gloo::events::EventListener;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlInputElement};
use yew::prelude::*;

mod components;
use components::{search_result_component, SearchResult};

const MIN_CHARS: usize = 2;

#[wasm_bindgen(module = "/public/glue.js")]
extern "C" {
    #[wasm_bindgen(js_name = invokeSearch, catch)]
    pub async fn run_search(query: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "onClearSearch")]
    pub async fn on_clear_search(callback: &Closure<dyn Fn()>);

    #[wasm_bindgen(js_name = "openResult", catch)]
    pub async fn open(url: String) -> Result<(), JsValue>;

    #[wasm_bindgen(js_name = "escape", catch)]
    pub async fn escape() -> Result<(), JsValue>;
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::start_app::<App>();
}

#[function_component(App)]
pub fn app() -> Html {
    let search_results = use_state_eq(Vec::new);
    let selected_idx = use_state_eq(|| 0);
    let query = use_state_eq(|| "".to_string());

    {
        let selected_idx = selected_idx.clone();
        let search_results = search_results.clone();
        use_effect(move || {
            // Attach a keydown event listener to the document.
            let document = gloo::utils::document();
            let listener = EventListener::new(&document, "keydown", move |event| {
                let event = event.dyn_ref::<web_sys::KeyboardEvent>().unwrap_throw();
                if event.key() == "ArrowDown" {
                    event.stop_propagation();
                    selected_idx.set((*selected_idx + 1).min(10));
                } else if event.key() == "ArrowUp" {
                    event.stop_propagation();
                    selected_idx.set((*selected_idx - 1).max(0));
                } else if event.key() == "Enter" {
                    let selected: &SearchResult = (*search_results).get(*selected_idx).unwrap();
                    let url = selected.url.clone();
                    spawn_local(async move {
                        open(url).await.unwrap();
                    });
                } else if event.key() == "Escape" {
                    spawn_local(async move {
                        escape().await.unwrap();
                    });
                }
            });
            || drop(listener)
        });
    }

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

    // TODO: Is this the best way to handle calls from Tauri?
    let query_ref = query.clone();
    let results_ref = search_results.clone();
    spawn_local(async move {
        let cb = Closure::wrap(Box::new(move || {
            query_ref.set("".to_string());
            results_ref.set(Vec::new());
        }) as Box<dyn Fn()>);

        on_clear_search(&cb).await;
        cb.forget();
    });

    let results = search_results
        .iter()
        .enumerate()
        .map(|(idx, res)| search_result_component(res, idx == *selected_idx))
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
            <div class="query-container">
                <input
                    type={"text"}
                    class={"search-box"}
                    placeholder={"Spyglass Search"}
                    value={(*query).clone()}
                    {onkeyup}
                    spellcheck={"false"}
                    tabindex={"0"}
                />
            </div>
            <div class={"search-results-list"}>
                { results }
            </div>
        </div>
    }
}

fn update_results(handle: UseStateHandle<Vec<SearchResult>>, query: String) {
    if query.len() <= MIN_CHARS {
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
