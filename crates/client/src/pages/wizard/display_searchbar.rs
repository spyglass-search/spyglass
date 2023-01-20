use shared::event::ClientInvoke;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::{
    tauri_invoke,
    utils::{get_os, OsName},
};

#[derive(Properties, PartialEq)]
pub struct KeyElementProps {
    pub key_code: String,
}

#[function_component(Key)]
pub fn key_element(props: &KeyElementProps) -> Html {
    let classes = classes!(
        "mx-1",
        "px-1",
        "rounded",
        "border",
        "border-neutral-500",
        "bg-neutral-400",
        "text-black"
    );

    let code = match props.key_code.as_str() {
        "Cmd" | "Ctrl" | "CmdOrCtrl" => match get_os() {
            OsName::MacOS => "⌘",
            _ => "Ctrl",
        },
        _ => &props.key_code,
    };

    html! {
        <div class={classes}>{code}</div>
    }
}

pub fn parse_shortcut(shortcut: &str) -> Html {
    let keycodes: Vec<String> = shortcut.split('+').map(|k| k.to_string()).collect();

    let element: Html = keycodes
        .iter()
        .map(|k| {
            html! { <Key key_code={k.clone()} /> }
        })
        .collect();

    html! { <div class="px-2 flex flex-row">{element}</div> }
}

/// Confused about where the tray icon is?
#[function_component(DisplaySearchbarPage)]
pub fn display_search_help() -> Html {
    let shortcut = use_state(String::new);

    {
        let shortcut_state = shortcut.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    if let Ok(result) =
                        tauri_invoke::<_, String>(ClientInvoke::GetShortcut, "").await
                    {
                        shortcut_state.set(result);
                    }
                });

                || ()
            },
            (),
        );
    }

    html! {
        <div class="my-auto flex flex-col gap-4 items-center align-middle text-center">
            <div>
                <img src={"/launching-example.gif"} alt="Launching in action" class="mx-auto rounded-lg w-[196px]"/>
            </div>
            <div class="text-center text-sm">
                <div class="flex flex-row align-middle items-center text-white place-content-center">
                    {"Use "}{parse_shortcut(shortcut.as_str())}{" to show the searchbar."}
                </div>
                <div class="text-xs text-neutral-400">
                    {"You can change the shortcut in your settings."}
                </div>
            </div>
            <div class="text-center text-sm">
                <div class="flex flex-row align-middle items-center place-content-center text-white">
                    {"Use "}{parse_shortcut("Esc")}{" to hide the searchbar."}
                </div>
                <div class="text-xs text-neutral-400">
                    {"Clicking elsewhere on your screen will also hide the searchbar."}
                </div>
            </div>
        </div>
    }
}
