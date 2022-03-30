use gloo::events::EventListener;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlInputElement};
use yew::prelude::*;

mod components;
use components::{search_result_component, SearchResult};
mod events;

const LENS_SEARCH_PREFIX: &str = "/";

const MIN_CHARS: usize = 2;

const INPUT_HEIGHT: f64 = 80.0;
const RESULT_HEIGHT: f64 = 126.0;

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

    #[wasm_bindgen(js_name = "resizeWindow", catch)]
    pub fn resize_window(height: f64) -> Result<(), JsValue>;
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::start_app::<App>();
}

#[function_component(App)]
pub fn app() -> Html {
    // Lens related data + results
    let lens = use_state_eq(Vec::new);
    // Current query string
    let query = use_state_eq(|| "".to_string());
    // Search results + selected index
    let search_results = use_state_eq(Vec::new);
    let selected_idx = use_state_eq(|| 0);

    // Handle key events
    {
        let selected_idx = selected_idx.clone();
        let search_results = search_results.clone();
        let lens = lens.clone();
        let query = query.clone();

        use_effect(move || {
            // Attach a keydown event listener to the document.
            let document = gloo::utils::document();
            let listener = EventListener::new(&document, "keydown", move |event| {
                events::handle_global_key_down(event, lens.clone(), query.clone(), search_results.clone(), selected_idx.clone())
            });
            || drop(listener)
        });
    }

    // Handle changes to the query string
    {
        let search_results = search_results.clone();
        use_effect_with_deps(
            move |query| {
                if query.len() > MIN_CHARS {
                    if query.starts_with(LENS_SEARCH_PREFIX) {
                        // show lens search
                        log::info!("lens search: {}", query);
                        show_lens_results(search_results, query.clone())
                    } else {
                        log::info!("query search: {}", query);
                        update_results(search_results, query.clone());
                    }
                }
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
                <ul class={"lenses"}>
                    {lens.iter().map(|lens_name: &String| {
                        html! {
                            <li class={"lens"}>
                                <span class={"lens-title"}>{lens_name}</span>
                            </li>
                        }
                    }).collect::<Html>()}
                </ul>
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

fn clear_results(handle: UseStateHandle<Vec<SearchResult>>) {
    handle.set(Vec::new());
    resize_window(INPUT_HEIGHT).unwrap();
}

fn show_lens_results(handle: UseStateHandle<Vec<SearchResult>>, _: String) {
    let mut res = Vec::new();
    let test = SearchResult {
        title: "wiki".to_string(),
        description: "Search through a variety of wikis".to_string(),
        url: None
    };
    res.push(test);

    resize_window(INPUT_HEIGHT + (res.len() as f64) * RESULT_HEIGHT).unwrap();
    handle.set(res);
}

fn update_results(handle: UseStateHandle<Vec<SearchResult>>, query: String) {
    spawn_local(async move {
        match run_search(query).await {
            Ok(results) => {
                let results: Vec<SearchResult> = results.into_serde().unwrap();
                resize_window(INPUT_HEIGHT + (results.len() as f64) * RESULT_HEIGHT).unwrap();
                handle.set(results);
            }
            Err(e) => {
                let window = window().unwrap();
                window
                    .alert_with_message(&format!("Error: {:?}", e))
                    .unwrap();
                    clear_results(handle);
            }
        }
    })
}
