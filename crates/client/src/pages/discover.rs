use shared::event::ClientInvoke;
use shared::response::LensResult;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::function_component;
use yew::prelude::*;

use crate::components::{icons, Header};
// use crate::listen;
use crate::utils::RequestState;
use crate::{install_lens, invoke};
// use shared::event::ClientEvent;
use shared::response::InstallableLens;

async fn fetch_available_lenses() -> Option<Vec<LensResult>> {
    match invoke(ClientInvoke::ListInstallableLenses.as_ref(), JsValue::NULL).await {
        Ok(results) => match serde_wasm_bindgen::from_value::<Vec<InstallableLens>>(results) {
            Ok(lenses) => {
                let parsed: Vec<LensResult> = lenses
                    .iter()
                    .map(|lens| LensResult {
                        author: lens.author.clone(),
                        title: lens.name.clone(),
                        description: lens.description.clone(),
                        hash: lens.sha.clone(),
                        file_path: None,
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
pub struct LensProps {
    pub result: LensResult,
    #[prop_or_default]
    pub is_installed: bool,
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
    let component_styles = classes!("px-8", "py-4", "pr-0");
    let result = &props.result;

    let installed_el = if props.is_installed {
        html! {
            <div class="flex flex-row text-green-400 text-sm">
                <icons::BadgeCheckIcon />
                <div class="ml-2">{"Installed"}</div>
            </div>
        }
    } else {
        html! { <InstallButton download_url={result.download_url.clone().expect("Invalid lens download URL")} /> }
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

pub struct DiscoverPage {
    req_available: RequestState,
    installable: Vec<LensResult>,
}

pub enum Msg {
    FetchAvailable,
    SetAvailable(Option<Vec<LensResult>>),
}

impl Component for DiscoverPage {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();
        link.send_message(Msg::FetchAvailable);

        Self {
            req_available: RequestState::NotStarted,
            installable: Vec::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::FetchAvailable => {
                if self.req_available == RequestState::InProgress {
                    return false;
                }

                self.req_available = RequestState::InProgress;
                link.send_future(async { Msg::SetAvailable(fetch_available_lenses().await) });

                false
            }
            Msg::SetAvailable(_results) => false,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let _link = ctx.link();

        let contents = if self.req_available.is_done() {
            self.installable
                .iter()
                .map(|data| {
                    html! {<Lens result={data.clone()} is_installed={false} /> }
                })
                .collect::<Html>()
        } else {
            html! {
                <div class="flex justify-center">
                    {"to be completed"}
                </div>
            }
        };

        html! {
            <div>
                <Header label="Discover" />
                <div>{contents}</div>
            </div>
        }
    }
}
