use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use yew::function_component;
use yew::prelude::*;

use shared::response::PluginResult;

use crate::components::icons;
use crate::invoke;
use crate::utils::RequestState;

fn fetch_installed_plugins(
    plugins_handle: UseStateHandle<Vec<PluginResult>>,
    req_state: UseStateHandle<RequestState>,
) {
    spawn_local(async move {
        match invoke("list_plugins", JsValue::NULL).await {
            Ok(results) => {
                plugins_handle.set(results.into_serde().unwrap());
                req_state.set(RequestState::Finished);
            }
            Err(e) => {
                log::info!("Error fetching plugins: {:?}", e);
                req_state.set(RequestState::Error);
            }
        }
    });
}

#[function_component(PluginManagerPage)]
pub fn plugin_manager_page() -> Html {
    let req_state = use_state_eq(|| RequestState::NotStarted);
    let plugins: UseStateHandle<Vec<PluginResult>> = use_state_eq(Vec::new);

    if *req_state == RequestState::NotStarted {
        req_state.set(RequestState::InProgress);
        fetch_installed_plugins(plugins.clone(), req_state.clone());
    }

    let contents = if req_state.is_done() {
        html! {
            <>
            {
                plugins.iter().map(|_| {
                    html! { <span>{"PLUGIN!"}</span> }
                }).collect::<Html>()
            }
            </>
        }
    } else {
        html! {
            <div class="flex justify-center">
                <div class="p-16">
                    <icons::RefreshIcon height={"h-16"} width={"w-16"} animate_spin={true} />
                </div>
            </div>
        }
    };

    html! {
        <div class="text-white">
            {contents}
        </div>
    }
}
