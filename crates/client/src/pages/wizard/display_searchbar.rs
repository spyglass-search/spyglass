use shared::event::ClientInvoke;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::btn;
use crate::tauri_invoke;
use super::WizardPageProps;

#[derive(Properties, PartialEq)]
pub struct KeyElementProps {
    pub key_code: String
}

#[function_component(Key)]
pub fn key_element(props: &KeyElementProps) -> Html {
    let is_mac = true;
    let classes = classes!(
        "mx-1",
        "px-1",
        "rounded",
        "border",
        "border-neutral-500",
        "bg-neutral-400",
        "text-black",
        "text-xl",
    );

    let code = match props.key_code.as_str() {
        "Cmd" | "Ctrl" | "CmdOrCtrl" => {
            if is_mac {
                "⌘"
            } else {
                "^"
            }
        },
        _ => &props.key_code
    };

    html! {
        <div class={classes}>{code}</div>
    }
}

pub fn parse_shortcut(shortcut: &str) -> Html {
    let keycodes: Vec<String> = shortcut.split('+').map(|k| k.to_string()).collect();

    let element: Html = keycodes.iter()
        .map(|k| {
            html!{ <Key key_code={k.clone()} /> }
        }).collect();

    html!{ <div class="px-2 flex flex-row">{element}</div> }
}

/// Confused about where the tray icon is?
#[function_component(DisplaySearchbarPage)]
pub fn display_search_help(props: &WizardPageProps) -> Html {
    let shortcut = use_state(String::new);

    {
        let shortcut_state = shortcut.clone();
        use_effect_with_deps(move |_| {
            spawn_local(async move {
                if let Ok(result) = tauri_invoke::<_, String>(ClientInvoke::GetShortcut, "").await {
                    shortcut_state.set(result);
                }
            });

            || ()
        }, ());

    }

    html! {
        <div class="py-4 px-8 text-neutral-400 bg-neutral-800 h-screen text-center flex flex-col gap-4">
            <div class="mt-8 flex flex-col gap-8">
                <div class="mx-auto flex flex-row items-center align-middle">
                    {"Use "}{parse_shortcut(shortcut.as_str())}{" to open the searchbar"}
                </div>
                <div class="text-sm">
                    {"You can change the shortcut in your settings. The searchbar is also accessible via the menubar menu."}
                </div>
            </div>
            <div class="mt-auto mb-4 flex flex-col gap-4">
                <btn::Btn onclick={props.on_next.clone()}>
                    {"Indexing files, web content, and more."}
                </btn::Btn>
                <btn::Btn _type={btn::BtnType::Danger} onclick={props.on_cancel.clone()}>
                    {"Stop the wizard, I'm a seasoned expert."}
                </btn::Btn>
            </div>
        </div>
    }
}
