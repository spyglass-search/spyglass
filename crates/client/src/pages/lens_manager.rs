use std::collections::HashSet;
use shared::response::LensResult;
use wasm_bindgen_futures::spawn_local;
use yew::function_component;
use yew::prelude::*;

use crate::components::icons;
use crate::{installable_lenses, installed_lenses};
use shared::response::InstallableLens;

#[derive(Properties, PartialEq)]
pub struct LensProps {
    pub result: LensResult,
    #[prop_or_default]
    pub is_installed: bool,
}

fn fetch_installed_lenses(lenses_handle: UseStateHandle<Vec<LensResult>>) {
    spawn_local(async move {
        match installed_lenses().await {
            Ok(results) => {
                lenses_handle.set(results.into_serde().unwrap());
            }
            Err(e) => {
                log::info!("Error: {:?}", e);
            }
        }
    });
}

fn fetch_installable_lenses(data_handle: UseStateHandle<Vec<LensResult>>) {
    spawn_local(async move {
        match installable_lenses().await {
            Ok(results) => {
                let lenses: Vec<InstallableLens> = results.into_serde().unwrap();
                let parsed: Vec<LensResult> = lenses
                    .iter()
                    .map(|lens| LensResult {
                        author: lens.author.to_owned(),
                        title: lens.name.to_owned(),
                        description: lens.description.to_owned(),
                    })
                    .collect();

                data_handle.set(parsed);
            }
            Err(e) => {
                log::info!("Error: {:?}", e);
            }
        }
    });
}

#[function_component(Lens)]
pub fn lens_component(props: &LensProps) -> Html {
    let component_styles: Vec<String> = vec![
        "border-t".into(),
        "border-neutral-600".into(),
        "p-4".into(),
        "pr-0".into(),
        "text-white".into(),
        "bg-netural-800".into(),
    ];

    let installed_el = if props.is_installed {
        html! {
            <a class="flex flex-row text-green-400 text-sm">
                <icons::BadgeCheckIcon />
                <div class="ml-2">{"Installed"}</div>
            </a>
        }
    } else {
        html! {
            <a class="flex flex-row text-cyan-400 text-sm">
                <icons::DocumentDownloadIcon />
                <div class="ml-2">{"Install"}</div>
            </a>
        }
    };

    let result = &props.result;
    html! {
        <div class={component_styles}>
            <h2 class="text-xl truncate p-0">
                {result.title.clone()}
            </h2>
            <h2 class="text-xs truncate py-1 text-neutral-400">
                {"Crafted By:"}
                <a href={format!("https://github.com/{}", result.author)} target="_blank" class="ml-2 text-cyan-400">
                    {format!("@{}", result.author)}
                </a>
            </h2>
            <div class="leading-relaxed text-neutral-400 h-6 overflow-hidden text-ellipsis">
                {result.description.clone()}
            </div>
            <div class="pt-2 flex flex-row gap-8">
                {installed_el}
                <a class="flex flex-row text-neutral-400 text-sm">
                    <icons::EyeIcon />
                    <div class="ml-2">{"View Source"}</div>
                </a>
            </div>
        </div>
    }
}

#[function_component(LensManagerPage)]
pub fn lens_manager_page() -> Html {
    let user_installed: UseStateHandle<Vec<LensResult>> = use_state_eq(Vec::new);
    let installable: UseStateHandle<Vec<LensResult>> = use_state_eq(Vec::new);

    let ui_req_finished = use_state(|| false);
    if user_installed.is_empty() && !(*ui_req_finished) {
        ui_req_finished.set(true);
        fetch_installed_lenses(user_installed.clone());
    }

    let i_req_finished = use_state(|| false);
    if installable.is_empty() && !(*i_req_finished) {
        i_req_finished.set(true);
        fetch_installable_lenses(installable.clone());
    }

    let on_open_folder = {
        move |_| {
            spawn_local(async {
                let _ = crate::open_lens_folder().await;
            });
        }
    };

    let already_installed: HashSet<String> = user_installed.iter().map(|x| x.title.clone()).collect();
    installable.set(installable
        .iter()
        .filter(|x| !already_installed.contains(&x.title))
        .map(|x| x.to_owned())
        .collect::<Vec<LensResult>>());

    html! {
        <div class="text-white">
            <div class="pt-4 px-8 top-0 sticky bg-stone-900 z-400 h-20">
                <div class="flex flex-row items-center gap-4">
                    <h1 class="text-2xl grow">{"Lens Manager"}</h1>
                    <button
                        onclick={on_open_folder}
                        class="flex flex-row border border-neutral-600 rounded-lg p-2 active:bg-neutral-700 hover:bg-neutral-600 text-sm">
                        <icons::FolderOpenIcon />
                        <div class="ml-2">{"Lens folder"}</div>
                    </button>
                    <button
                        class="border border-neutral-600 rounded-lg p-2 active:bg-neutral-700 hover:bg-neutral-600">
                        <icons::RefreshIcon />
                    </button>
                </div>
            </div>
            <div class="px-8">
                {
                    user_installed.iter().map(|data| {
                        html! {<Lens result={data.clone()} is_installed={true} /> }
                    }).collect::<Html>()
                }
                {
                    installable.iter().map(|data| {
                        html! {<Lens result={data.clone()} is_installed={false} /> }
                    }).collect::<Html>()
                }
            </div>
        </div>
    }
}
