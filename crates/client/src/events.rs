use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{Element, HtmlElement, HtmlInputElement};
use yew::prelude::*;

use super::{invoke, open};
use crate::components::ResultListData;
use crate::constants;
use crate::pages::{clear_results, show_doc_results, show_lens_results};
use shared::event::ClientInvoke;

fn scroll_to_result(idx: usize) {
    let document = gloo::utils::document();
    if let Some(el) = document.get_element_by_id(&format!("result-{}", idx)) {
        let el: HtmlElement = el.unchecked_into();
        el.scroll_into_view();
    }
}

pub fn handle_global_key_down(
    event: &Event,
    node_ref: NodeRef,
    lens: UseStateHandle<Vec<String>>,
    query: UseStateHandle<String>,
    query_ref: NodeRef,
    search_results: UseStateHandle<Vec<ResultListData>>,
    selected_idx: UseStateHandle<usize>,
) {
    let event = event.dyn_ref::<web_sys::KeyboardEvent>().unwrap_throw();
    // Search result navigation
    if event.key() == "ArrowDown" {
        event.stop_propagation();
        let max_len = if search_results.is_empty() {
            0
        } else {
            search_results.len() - 1
        };
        selected_idx.set((*selected_idx + 1).min(max_len));
        scroll_to_result(*selected_idx);
    } else if event.key() == "ArrowUp" {
        event.stop_propagation();
        let new_idx = (*selected_idx).max(1) - 1;
        selected_idx.set(new_idx);
        scroll_to_result(new_idx);
    } else if event.key() == "Enter" {
        let selected: &ResultListData = (*search_results).get(*selected_idx).unwrap();
        if let Some(url) = selected.url.clone() {
            spawn_local(async move {
                open(url).await.unwrap();
            });
        // Otherwise we're dealing w/ a lens, add to lens vec
        } else {
            // Add lens to list
            let mut new_lens = lens.to_vec();
            new_lens.push(selected.title.to_string());
            lens.set(new_lens);
            // Clear query string
            query.set("".to_string());
            // Clear results list
            let el = node_ref.cast::<Element>().unwrap();
            clear_results(search_results, el);

            let el = query_ref.cast::<HtmlInputElement>().unwrap();
            el.set_value("");
        }
    } else if event.key() == "Escape" {
        spawn_local(async move {
            let _ = invoke(ClientInvoke::Escape.as_ref(), JsValue::NULL).await;
        });
    } else if event.key() == "Backspace" {
        event.stop_propagation();
        if query.is_empty() && !lens.is_empty() {
            log::info!("updating lenses");
            let all_but_last = lens[0..lens.len() - 1].to_vec();
            lens.set(all_but_last);
        }

        if query.len() < crate::constants::MIN_CHARS {
            // Clear results list
            let el = node_ref.cast::<Element>().unwrap();
            clear_results(search_results, el);
        }
    }
}

pub fn handle_query_change(
    query: &str,
    node_ref: NodeRef,
    lens: UseStateHandle<Vec<String>>,
    search_results: UseStateHandle<Vec<ResultListData>>,
    selected_idx: UseStateHandle<usize>,
) {
    let el = node_ref.cast::<Element>().unwrap();
    if query.starts_with(constants::LENS_SEARCH_PREFIX) {
        // show lens search
        return show_lens_results(search_results, el, selected_idx, query.to_string());
    }

    if query.len() >= constants::MIN_CHARS {
        show_doc_results(search_results, &lens, el, selected_idx, query.to_string())
    }
}
