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

async fn fetch_user_installed_lenses() -> Option<Vec<LensResult>> {
    match invoke(ClientInvoke::ListInstalledLenses.as_ref(), JsValue::NULL).await {
        Ok(results) => match results.into_serde() {
            Ok(parsed) => Some(parsed),
            Err(e) => {
                log::error!("Unable to deserialize results: {}", e.to_string());
                None
            }
        },
        Err(e) => {
            log::error!("Error fetching lenses: {:?}", e);
            None
        }
    }
}

async fn fetch_available_lenses() -> Option<Vec<LensResult>> {
    match invoke(ClientInvoke::ListInstallableLenses.as_ref(), JsValue::NULL).await {
        Ok(results) => match results.into_serde::<Vec<InstallableLens>>() {
            Ok(lenses) => {
                let parsed: Vec<LensResult> = lenses
                    .iter()
                    .map(|lens| LensResult {
                        author: lens.author.clone(),
                        title: lens.name.clone(),
                        description: lens.description.clone(),
                        hash: lens.sha.clone(),
                        html_url: Some(lens.html_url.clone()),
                        download_url: Some(lens.download_url.clone()),
                    })
                    .collect();

                Some(parsed)
            }
            Err(e) => {
                log::error!("Unable to deserialize results: {}", e);
                None
            }
        },
        Err(e) => {
            log::error!("Error: {:?}", e);
            None
        }
    }
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

pub struct LensManagerPage {
    active_tab: usize,
    req_user_installed: RequestState,
    req_available: RequestState,
    user_installed: Vec<LensResult>,
    installable: Vec<LensResult>,
}
pub enum Msg {
    RunLensUpdate,
    RunOpenFolder,
    RunRefresher,
    SetActiveTab(usize),
    SetUserInstalled(Option<Vec<LensResult>>),
    SetAvailable(Option<Vec<LensResult>>),
}

impl LensManagerPage {
    fn available_lenses_tabview(&self) -> Html {
        // Filter already installed lenses from list of available
        let already_installed: HashSet<String> = self
            .user_installed
            .iter()
            .map(|x| x.title.clone())
            .collect();
        let installable = self
            .installable
            .iter()
            .filter(|x| !already_installed.contains(&x.title))
            .map(|x| x.to_owned())
            .collect::<Vec<LensResult>>();

        installable
            .iter()
            .map(|data| {
                html! {<Lens result={data.clone()} is_installed={false} /> }
            })
            .collect::<Html>()
    }

    fn user_installed_tabview(&self) -> Html {
        self.user_installed
            .iter()
            .map(|data| {
                html! {<Lens result={data.clone()} is_installed={true} /> }
            })
            .collect::<Html>()
    }
}

impl Component for LensManagerPage {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();
        link.send_message(Msg::RunRefresher);

        // Handle refreshing the list
        {
            let link = link.clone();
            spawn_local(async move {
                let cb = Closure::wrap(Box::new(move |_| {
                    link.send_message(Msg::RunRefresher);
                }) as Box<dyn Fn(JsValue)>);

                let _ = listen(ClientEvent::RefreshLensManager.as_ref(), &cb).await;
                cb.forget();
            });
        }

        Self {
            active_tab: 0,
            req_user_installed: RequestState::NotStarted,
            req_available: RequestState::NotStarted,
            user_installed: Vec::new(),
            installable: Vec::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::SetActiveTab(idx) => {
                self.active_tab = idx;
                true
            }
            Msg::RunOpenFolder => {
                spawn_local(async {
                    let _ = invoke(ClientInvoke::OpenLensFolder.as_ref(), JsValue::NULL).await;
                });

                false
            }
            Msg::RunLensUpdate => {
                spawn_local(async {
                    let _ = invoke(ClientInvoke::RunLensUpdater.as_ref(), JsValue::NULL).await;
                });

                false
            }
            Msg::RunRefresher => {
                // Don't run if requests are in flight.
                if self.req_available == RequestState::InProgress
                    || self.req_user_installed == RequestState::InProgress
                {
                    return false;
                }

                self.req_user_installed = RequestState::InProgress;
                self.req_available = RequestState::InProgress;

                link.send_future(async { Msg::SetAvailable(fetch_available_lenses().await) });
                link.send_future(async {
                    Msg::SetUserInstalled(fetch_user_installed_lenses().await)
                });

                false
            }
            Msg::SetAvailable(lenses) => {
                if let Some(lenses) = lenses {
                    self.req_available = RequestState::Finished;
                    self.installable = lenses;
                    true
                } else {
                    self.req_available = RequestState::Error;
                    false
                }
            }
            Msg::SetUserInstalled(lenses) => {
                if let Some(lenses) = lenses {
                    self.req_user_installed = RequestState::Finished;
                    self.user_installed = lenses;
                    true
                } else {
                    self.req_user_installed = RequestState::Error;
                    false
                }
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let contents = if self.req_user_installed.is_done() && self.req_available.is_done() {
            if self.active_tab == 0 {
                self.user_installed_tabview()
            } else {
                self.available_lenses_tabview()
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

        let tabs = html! {
            <Tabs
                onchange={link.callback(|e: TabEvent| Msg::SetActiveTab(e.tab_idx))}
                tabs={vec!["Installed".to_string(), "Available".to_string()]}
            />
        };

        html! {
            <div class="text-white relative">
                <Header label="Lens Manager" tabs={tabs}>
                    <Btn onclick={link.callback(|_| Msg::RunOpenFolder)}>
                        <icons::FolderOpenIcon />
                        <div class="ml-2">{"Lens folder"}</div>
                    </Btn>
                    <Btn onclick={link.callback(|_| Msg::RunLensUpdate)}>
                        <icons::RefreshIcon />
                    </Btn>
                    <Btn onclick={link.callback(|_| Msg::RunRefresher)}>
                        <icons::RefreshIcon />
                    </Btn>
                </Header>
                <div>{contents}</div>
            </div>
        }
    }
}
