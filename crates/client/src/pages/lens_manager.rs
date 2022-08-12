use shared::event::ClientInvoke;
use shared::response::LensResult;
use std::collections::HashSet;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::function_component;
use yew::prelude::*;

use crate::components::{btn::Btn, icons, Header, TabEvent, Tabs};
use crate::listen;
use crate::utils::RequestState;
use crate::{install_lens, invoke};
use shared::event::ClientEvent;
use shared::response::InstallableLens;

#[derive(Properties, PartialEq, Eq)]
pub struct LensProps {
    pub result: LensResult,
    #[prop_or_default]
    pub is_installed: bool,
}

fn fetch_installed_lenses(
    lenses_handle: UseStateHandle<Vec<LensResult>>,
    req_state: UseStateHandle<RequestState>,
) {
    spawn_local(async move {
        match invoke(ClientInvoke::ListInstalledLenses.as_ref(), JsValue::NULL).await {
            Ok(results) => {
                lenses_handle.set(results.into_serde().unwrap());
                req_state.set(RequestState::Finished);
            }
            Err(e) => {
                log::info!("Error fetching lenses: {:?}", e);
                req_state.set(RequestState::Error);
            }
        }
    });
}

fn fetch_installable_lenses(
    data_handle: UseStateHandle<Vec<LensResult>>,
    req_state: UseStateHandle<RequestState>,
) {
    spawn_local(async move {
        match invoke(ClientInvoke::ListInstallableLenses.as_ref(), JsValue::NULL).await {
            Ok(results) => {
                let lenses: Vec<InstallableLens> = results.into_serde().unwrap();
                let parsed: Vec<LensResult> = lenses
                    .iter()
                    .map(|lens| LensResult {
                        author: lens.author.to_owned(),
                        title: lens.name.to_owned(),
                        description: lens.description.to_owned(),
                        html_url: Some(lens.html_url.to_owned()),
                        download_url: Some(lens.download_url.to_owned()),
                    })
                    .collect();

                data_handle.set(parsed);
                req_state.set(RequestState::Finished);
            }
            Err(e) => {
                log::info!("Error: {:?}", e);
                req_state.set(RequestState::Error);
            }
        }
    });
}

#[derive(Properties, PartialEq, Eq)]
pub struct InstallBtnProps {
    pub download_url: String,
}

#[function_component(InstallButton)]
pub fn install_btn(props: &InstallBtnProps) -> Html {
    let is_installing = use_state_eq(|| false);
    let download_url = props.download_url.clone();

    let onclick = {
        let is_installing = is_installing.clone();
        Callback::from(move |_| {
            let download_url = download_url.clone();
            is_installing.set(true);
            // Download to lens directory
            spawn_local(async move {
                if let Err(e) = install_lens(download_url.clone()).await {
                    log::error!("error installing lens: {} {:?}", download_url.clone(), e);
                }
            });
        })
    };

    if *is_installing {
        html! {
            <div class="flex flex-row text-cyan-400 text-sm cursor-pointer hover:text-white">
                <icons::RefreshIcon animate_spin={true} />
                <div class="ml-2">{"Installing"}</div>
            </div>
        }
    } else {
        html! {
            <button
                {onclick}
                class="flex flex-row text-cyan-400 text-sm cursor-pointer hover:text-white">
                <icons::DocumentDownloadIcon />
                <div class="ml-2">{"Install"}</div>
            </button>
        }
    }
}

#[function_component(Lens)]
pub fn lens_component(props: &LensProps) -> Html {
    let component_styles: Vec<String> = vec![
        "border-t".into(),
        "border-neutral-600".into(),
        "px-8".into(),
        "py-4".into(),
        "pr-0".into(),
        "text-white".into(),
        "bg-netural-800".into(),
    ];
    let result = &props.result;

    let installed_el = if props.is_installed {
        html! {
            <div class="flex flex-row text-green-400 text-sm">
                <icons::BadgeCheckIcon />
                <div class="ml-2">{"Installed"}</div>
            </div>
        }
    } else {
        html! { <InstallButton download_url={result.download_url.clone().unwrap()} /> }
    };

    let view_link = if result.html_url.is_some() {
        html! {
            <a href={result.html_url.clone()} target="_blank" class="flex flex-row text-neutral-400 text-sm cursor-pointer hover:text-white">
                <icons::EyeIcon />
                <div class="ml-2">{"View Source"}</div>
            </a>
        }
    } else {
        html! {}
    };

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
                {view_link}
            </div>
        </div>
    }
}

#[function_component(LensManagerPage)]
pub fn lens_manager_page() -> Html {
    let active_tab = use_state_eq(|| 0);
    let user_installed: UseStateHandle<Vec<LensResult>> = use_state_eq(Vec::new);
    let installable: UseStateHandle<Vec<LensResult>> = use_state_eq(Vec::new);

    let ui_req_state = use_state_eq(|| RequestState::NotStarted);
    if *ui_req_state == RequestState::NotStarted {
        ui_req_state.set(RequestState::InProgress);
        fetch_installed_lenses(user_installed.clone(), ui_req_state.clone());
    }

    let i_req_state = use_state_eq(|| RequestState::NotStarted);
    if *i_req_state == RequestState::NotStarted {
        i_req_state.set(RequestState::InProgress);
        fetch_installable_lenses(installable.clone(), i_req_state.clone());
    }

    let on_open_folder = Callback::from(move |_| {
        spawn_local(async {
            let _ = invoke(ClientInvoke::OpenLensFolder.as_ref(), JsValue::NULL).await;
        });
    });

    let on_refresh = {
        let ui_req_state = ui_req_state.clone();
        let i_req_state = i_req_state.clone();
        Callback::from(move |_| {
            ui_req_state.set(RequestState::NotStarted);
            i_req_state.set(RequestState::NotStarted);
        })
    };

    let already_installed: HashSet<String> =
        user_installed.iter().map(|x| x.title.clone()).collect();
    installable.set(
        installable
            .iter()
            .filter(|x| !already_installed.contains(&x.title))
            .map(|x| x.to_owned())
            .collect::<Vec<LensResult>>(),
    );

    // Handle refreshing the list
    {
        let ui_req_state = ui_req_state.clone();
        let i_req_state = i_req_state.clone();
        spawn_local(async move {
            let cb = Closure::wrap(Box::new(move |_| {
                ui_req_state.set(RequestState::NotStarted);
                i_req_state.set(RequestState::NotStarted);
            }) as Box<dyn Fn(JsValue)>);

            let _ = listen(ClientEvent::RefreshLensManager.as_ref(), &cb).await;
            cb.forget();
        });
    }

    let contents = if ui_req_state.is_done() && i_req_state.is_done() {
        if *active_tab == 0 {
            html! {
                <>
                {
                    user_installed.iter().map(|data| {
                        html! {<Lens result={data.clone()} is_installed={true} /> }
                    }).collect::<Html>()
                }
                </>
            }
        } else {
            html! {
                <>
                {
                    installable.iter().map(|data| {
                        html! {<Lens result={data.clone()} is_installed={false} /> }
                    }).collect::<Html>()
                }
            </>
            }
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

    let handle_tab_change = Callback::from(move |evnt: TabEvent| {
        active_tab.set(evnt.tab_idx);
    });

    let tabs = html! {
        <Tabs onchange={handle_tab_change} tabs={vec!["Installed".to_string(), "Uninstalled".to_string()]} />
    };

    html! {
        <div class="text-white relative">
            <Header label="Lens Manager" tabs={tabs}>
                <Btn onclick={on_open_folder}>
                    <icons::FolderOpenIcon />
                    <div class="ml-2">{"Lens folder"}</div>
                </Btn>
                <Btn onclick={on_refresh}>
                    <icons::RefreshIcon />
                </Btn>
            </Header>
            <div>{contents}</div>
        </div>
    }
}
