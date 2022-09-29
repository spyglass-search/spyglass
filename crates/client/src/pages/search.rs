use gloo::events::EventListener;
use gloo::timers::callback::Timeout;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, Element, HtmlElement, HtmlInputElement};
use yew::prelude::*;

use shared::{event::ClientEvent, response};

use crate::components::{ResultListData, SearchResultItem, SelectedLens};
use crate::events;
use crate::{listen, resize_window, search_docs, search_lenses};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "clearTimeout")]
    fn clear_timeout(handle: i32);
}

type TimeoutId = i32;
const QUERY_DEBOUNCE_MS: u32 = 256;

#[function_component(SearchPage)]
pub fn search_page() -> Html {
    // Lens related data + results
    let lens = use_state_eq(Vec::new);
    // Current query string
    let query = use_state_eq(|| "".to_string());
    let query_ref = use_node_ref();

    // Search results + selected index
    let search_results = use_state_eq(Vec::new);
    let selected_idx = use_state_eq(|| 0);

    let node_ref = use_node_ref();
    let query_debounce: UseStateHandle<Option<TimeoutId>> = use_state(|| None);

    // Handle key events
    {
        let selected_idx = selected_idx.clone();
        let search_results = search_results.clone();
        let lens = lens.clone();
        let query = query.clone();
        let query_ref = query_ref.clone();
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
                    query_ref.clone(),
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
                if let Some(timeout_id) = *query_debounce {
                    clear_timeout(timeout_id);
                    query_debounce.set(None);
                }

                let query = query.clone();
                let handle = Timeout::new(QUERY_DEBOUNCE_MS, move || {
                    events::handle_query_change(
                        &query,
                        node_ref,
                        lens,
                        search_results,
                        selected_idx,
                    )
                });

                let id = handle.forget();
                query_debounce.set(Some(id));
                || ()
            },
            (*query).clone(),
        );
    }

    // Handle callbacks to Tauri
    // TODO: Is this the best way to handle calls from Tauri?
    {
        let node_clone = node_ref.clone();
        let lens = lens.clone();
        let query = query.clone();
        let query_ref = query_ref.clone();
        let results = search_results.clone();
        let selected_idx = selected_idx.clone();
        // Reset query string, results list, etc when we receive a "clear_search"
        // event from tauri
        spawn_local(async move {
            let cb = Closure::wrap(Box::new(move |_| {
                query.set("".to_string());
                results.set(Vec::new());
                selected_idx.set(0);
                lens.set(Vec::new());

                let el = query_ref.cast::<HtmlInputElement>().unwrap();
                el.set_value("");

                let node = node_clone.cast::<Element>().unwrap();
                spawn_local(async move {
                    resize_window(node.client_height() as f64).await.unwrap();
                });
            }) as Box<dyn Fn(JsValue)>);

            let _ = listen(ClientEvent::ClearSearch.as_ref(), &cb).await;
            cb.forget();
        });

        let node_clone = node_ref.clone();
        // Focus on the search box when we receive an "focus_window" event from
        // tauri
        spawn_local(async move {
            let cb = Closure::wrap(Box::new(move |_| {
                let document = gloo::utils::document();
                if let Some(el) = document.get_element_by_id("searchbox") {
                    let el: HtmlElement = el.unchecked_into();
                    let _ = el.focus();
                }

                if let Some(node) = node_clone.cast::<Element>() {
                    spawn_local(async move {
                        resize_window(node.client_height() as f64).await.unwrap();
                    });
                }
            }) as Box<dyn Fn(JsValue)>);
            let _ = listen(ClientEvent::FocusWindow.as_ref(), &cb).await;
            cb.forget();
        });
    }

    {
        // Refresh search results
        let query = query.clone();
        spawn_local(async move {
            let cb = Closure::wrap(Box::new(move |_| {
                let document = gloo::utils::document();
                if let Some(el) = document.get_element_by_id("searchbox") {
                    let el: HtmlInputElement = el.unchecked_into();
                    query.set("".into());
                    query.set(el.value());
                }
            }) as Box<dyn Fn(JsValue)>);
            let _ = listen(ClientEvent::RefreshSearchResults.as_ref(), &cb).await;
            cb.forget();
        });
    }

    let results = search_results
        .iter()
        .enumerate()
        .map(|(idx, res)| {
            let is_selected = idx == *selected_idx;
            html! {
                <SearchResultItem id={format!("result-{}", idx)} result={res.clone()} {is_selected} />
            }
        })
        .collect::<Html>();

    let onkeyup = {
        Callback::from(move |e: KeyboardEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            query.set(input.value());
        })
    };

    let onkeydown = {
        Callback::from(move |e: KeyboardEvent| {
            let key = e.key();
            match key.as_str() {
                // Prevent cursor from moving around
                "ArrowUp" => e.prevent_default(),
                "ArrowDown" => e.prevent_default(),
                // Prevent search box from losing focus
                "Tab" => e.prevent_default(),
                _ => (),
            }
        })
    };

    html! {
        <div ref={node_ref} class="relative overflow-hidden rounded-xl">
            <div class="flex flex-nowrap w-full">
                <SelectedLens lens={(*lens).clone()} />
                <input
                    ref={query_ref}
                    id="searchbox"
                    type="text"
                    class="bg-neutral-800 text-white text-5xl py-4 px-6 overflow-hidden flex-1 outline-none active:outline-none focus:outline-none"
                    placeholder="Search"
                    {onkeyup}
                    {onkeydown}
                    spellcheck="false"
                    tabindex="-1"
                />
            </div>
            <div class="overflow-y-auto overflow-x-hidden h-full">{ results }</div>
        </div>
    }
}

pub fn clear_results(handle: UseStateHandle<Vec<ResultListData>>, node: Element) {
    handle.set(Vec::new());
    spawn_local(async move {
        resize_window(node.client_height() as f64).await.unwrap();
    });
}

pub fn show_lens_results(
    handle: UseStateHandle<Vec<ResultListData>>,
    node: Element,
    selected_idx: UseStateHandle<usize>,
    query: String,
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
                spawn_local(async move {
                    resize_window(node.client_height() as f64).await.unwrap();
                });
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

pub fn show_doc_results(
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
                spawn_local(async move {
                    resize_window(node.client_height() as f64).await.unwrap();
                });
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
