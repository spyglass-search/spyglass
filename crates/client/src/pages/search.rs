use gloo::events::EventListener;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, Element, HtmlElement, HtmlInputElement};
use yew::prelude::*;

use shared::response;

use crate::components::{ResultListData, SearchResultItem, SelectedLens};
use crate::events;
use crate::{on_clear_search, on_focus, resize_window, search_docs, search_lenses};

#[function_component(DeleteButton)]
fn delete_btn() -> Html {
    html! {
        <div class="float-right pl-4 pr-0 h-28">
            <button class="text-red-600">
                <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
            </button>
        </div>
    }
}

#[function_component(SearchPage)]
pub fn search_page() -> Html {
    // Lens related data + results
    let lens = use_state_eq(Vec::new);
    // Current query string
    let query = use_state_eq(|| "".to_string());
    let query_ref = use_state_eq(NodeRef::default);

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
                events::handle_query_change(query, node_ref, lens, search_results, selected_idx);
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
            let cb = Closure::wrap(Box::new(move || {
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
            }) as Box<dyn Fn()>);

            on_clear_search(&cb).await;
            cb.forget();
        });

        let node_clone = node_ref.clone();
        // Focus on the search box when we receive an "focus_window" event from
        // tauri
        spawn_local(async move {
            let cb = Closure::wrap(Box::new(move || {
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
            }) as Box<dyn Fn()>);
            on_focus(&cb).await;
            cb.forget();
        });
    }

    let results = search_results
        .iter()
        .enumerate()
        .map(|(idx, res)| {
            html! {
                <SearchResultItem result={res.clone()} is_selected={idx == *selected_idx} />
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
        <div ref={(*node_ref).clone()}>
            <div class={"flex flex-nowrap w-full"}>
                <SelectedLens lens={(*lens).clone()} />
                <input
                    ref={(*query_ref).clone()}
                    id={"searchbox"}
                    type={"text"}
                    class={"bg-neutral-800 text-white text-5xl p-4 overflow-hidden flex-1 focus:outline-none"}
                    placeholder={"Search"}
                    {onkeyup}
                    {onkeydown}
                    spellcheck={"false"}
                    tabindex={"-1"}
                />
            </div>
            <div>{ results }</div>
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
