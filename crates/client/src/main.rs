use gloo::events::EventListener;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, Element, HtmlInputElement};
use yew::prelude::*;

mod components;
mod events;
use components::{search_result_component, selected_lens_list, ResultListData};
use shared::response;

const LENS_SEARCH_PREFIX: &str = "/";
const MIN_CHARS: usize = 2;

#[wasm_bindgen(module = "/public/glue.js")]
extern "C" {
    #[wasm_bindgen(js_name = "searchDocs", catch)]
    pub async fn search_docs(lenses: JsValue, query: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "searchLenses", catch)]
    pub async fn search_lenses(query: String) -> Result<JsValue, JsValue>;

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
    let node_ref = use_state_eq(NodeRef::default);

    // Handle key events
    {
        let selected_idx = selected_idx.clone();
        let search_results = search_results.clone();
        let lens = lens.clone();
        let query = query.clone();
        let node_ref = node_ref.clone();

        use_effect(move || {
            // Attach a keydown event listener to the document.
            let document = gloo::utils::document();
            let listener = EventListener::new(&document, "keydown", move |event| {
                events::handle_global_key_down(
                    event,
                    node_ref.clone(),
                    lens.clone(),
                    query.clone(),
                    search_results.clone(),
                    selected_idx.clone(),
                )
            });
            || drop(listener)
        });
    }

    // Handle changes to the query string
    {
        let lens = lens.clone();
        let search_results = search_results.clone();
        let selected_idx = selected_idx.clone();
        let node_ref = node_ref.clone();
        use_effect_with_deps(
            move |query| {
                if query.len() > MIN_CHARS {
                    if query.starts_with(LENS_SEARCH_PREFIX) {
                        // show lens search
                        log::info!("lens search: {}", query);
                        let el = node_ref.cast::<Element>().unwrap();
                        show_lens_results(search_results, el, selected_idx, query.clone());
                    } else {
                        log::info!("query search: {}", query);
                        let el = node_ref.cast::<Element>().unwrap();
                        show_doc_results(search_results, &lens, el, selected_idx, query.clone());
                    }
                }
                || ()
            },
            (*query).clone(),
        );
    }

    {
        // TODO: Is this the best way to handle calls from Tauri?
        let lens = lens.clone();
        let query = query.clone();
        let results = search_results.clone();
        let selected_idx = selected_idx.clone();
        spawn_local(async move {
            let cb = Closure::wrap(Box::new(move || {
                query.set("".to_string());
                results.set(Vec::new());
                selected_idx.set(0);
                lens.set(Vec::new());
            }) as Box<dyn Fn()>);

            on_clear_search(&cb).await;
            cb.forget();
        });
    }

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
        <div ref={(*node_ref).clone()}>
            <div class="query-container">
                {selected_lens_list(&lens)}
                <input
                    type={"text"}
                    class={"search-box"}
                    placeholder={"Search"}
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

fn clear_results(handle: UseStateHandle<Vec<ResultListData>>, node: Element) {
    handle.set(Vec::new());
    resize_window(node.client_height() as f64).unwrap();
}

fn show_lens_results(
    handle: UseStateHandle<Vec<ResultListData>>,
    node: Element,
    selected_idx: UseStateHandle<usize>,
    query: String
) {
    let query = query.strip_prefix('/').unwrap().to_string();
    spawn_local(async move {
        match search_lenses(query).await {
            Ok(results) => {
                let results: Vec<response::LensResult> = results.into_serde().unwrap();
                let results = results
                    .iter()
                    .map(|x| x.into())
                    .collect::<Vec<ResultListData>>();

                let max_idx = results.len().max(1) - 1;
                if max_idx < *selected_idx {
                    selected_idx.set(max_idx);
                }

                handle.set(results);

                let height = node.client_height();
                resize_window(height as f64).unwrap();
            }
            Err(e) => {
                let window = window().unwrap();
                window
                    .alert_with_message(&format!("Error: {:?}", e))
                    .unwrap();
                clear_results(handle, node);
            }
        }
    })
}

fn show_doc_results(
    handle: UseStateHandle<Vec<ResultListData>>,
    lenses: &[String],
    node: Element,
    selected_idx: UseStateHandle<usize>,
    query: String,
) {
    let lenses = lenses.to_owned();
    spawn_local(async move {
        match search_docs(JsValue::from_serde(&lenses).unwrap(), query).await {
            Ok(results) => {
                let results: Vec<response::SearchResult> = results.into_serde().unwrap();
                let results = results
                    .iter()
                    .map(|x| x.into())
                    .collect::<Vec<ResultListData>>();


                let max_idx = results.len().max(1) - 1;
                if max_idx < *selected_idx {
                    selected_idx.set(max_idx);
                }

                handle.set(results);

                let height = node.client_height();
                resize_window(height as f64).unwrap();
            }
            Err(e) => {
                let window = window().unwrap();
                window
                    .alert_with_message(&format!("Error: {:?}", e))
                    .unwrap();
                clear_results(handle, node);
            }
        }
    })
}
