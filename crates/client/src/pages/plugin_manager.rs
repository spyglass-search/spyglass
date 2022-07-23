use shared::event::ClientEvent;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::function_component;
use yew::prelude::*;

use shared::event::ClientInvoke;
use shared::response::PluginResult;

use crate::components::icons;
use crate::utils::RequestState;
use crate::{invoke, listen, toggle_plugin};

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

#[derive(Properties, PartialEq)]
pub struct PluginProps {
    pub plugin: PluginResult,
}

#[function_component(Plugin)]
pub fn plugin_comp(props: &PluginProps) -> Html {
    let plugin = &props.plugin;
    let component_styles: Vec<String> = vec![
        "border-t".into(),
        "border-neutral-600".into(),
        "p-4".into(),
        "pr-0".into(),
        "text-white".into(),
        "bg-netural-800".into(),
    ];

    let btn_label = if plugin.is_enabled {
        "Disable"
    } else {
        "Enable"
    };

    let onclick = {
        let plugin_name = plugin.title.clone();
        Callback::from(move |_| {
            let plugin_name = plugin_name.clone();
            spawn_local(async move {
                if let Err(e) = toggle_plugin(&plugin_name).await {
                    log::error!("Error toggling plugin: {:?}", e);
                }
            })
        })
    };

    let on_edit_settings = {
        Callback::from(move |_| {
            spawn_local(async move {
                let _ = invoke(&ClientInvoke::EditPluginSettings.to_string(), JsValue::NULL).await;
            });
        })
    };

    let toggle_button = html! {
        <button
            onclick={onclick}
            class={vec!["flex", "flex-row", "text-sm", "cursor-pointer", "hover:text-white", if plugin.is_enabled { "text-red-400" } else { "text-green-400" }]}
        >
            <icons::LightningBoltIcon />
            <div class="ml-2">{btn_label}</div>
        </button>
    };

    let view_settings = html! {
        <button
            onclick={on_edit_settings}
            class="flex flex-row text-cyan-400 text-sm cursor-pointer hover:text-white"
        >
            <icons::PencilIcon />
            <div class="ml-2">{"Edit/View Settings"}</div>
        </button>
    };

    html! {
        <div class={component_styles}>
            <h2 class="text-xl truncate p-0">
                {plugin.title.clone()}
            </h2>
            <h2 class="text-xs truncate py-1 text-neutral-400">
                {"Crafted By:"}
                <span class="ml-2 text-cyan-400">{plugin.author.clone()}</span>
            </h2>
            <div class="leading-relaxed text-neutral-400 h-6 overflow-hidden text-ellipsis">
                {plugin.description.clone()}
            </div>
            <div class="pt-2 flex flex-row gap-8">
                {toggle_button}
                {view_settings}
            </div>
        </div>
    }
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
                plugins.iter()
                    .map(|plugin| html! { <Plugin plugin={plugin.clone()} /> })
                    .collect::<Html>()
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

    // Listen for updates from plugins
    {
        spawn_local(async move {
            let cb = Closure::wrap(Box::new(move || {
                log::info!("refresh!");
                req_state.set(RequestState::NotStarted);
            }) as Box<dyn Fn()>);

            let _ = listen(ClientEvent::RefreshPluginManager.as_ref(), &cb).await;
            cb.forget();
        });
    }

    html! {
        <div class="text-white">
            {contents}
        </div>
    }
}
