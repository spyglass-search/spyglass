use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use super::{escape, open};
use crate::components::SearchResult;

pub fn handle_global_key_down(
    event: &Event,
    lens: UseStateHandle<Vec<String>>,
    search_results: UseStateHandle<Vec<SearchResult>>,
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
    } else if event.key() == "ArrowUp" {
        event.stop_propagation();
        selected_idx.set((*selected_idx - 1).max(0));
    } else if event.key() == "Enter" {
        let selected: &SearchResult = (*search_results).get(*selected_idx).unwrap();
        if let Some(url) = selected.url.clone() {
            spawn_local(async move {
                open(url).await.unwrap();
            });
        }
    } else if event.key() == "Escape" {
        spawn_local(async move {
            escape().await.unwrap();
        });
    } else if event.key() == "Backspace" {
        event.stop_propagation();
        if !lens.is_empty() {
            log::info!("updating lenses");
            let all_but_last = lens[0..lens.len() - 1].to_vec();
            lens.set(all_but_last);
        }
    }
}
