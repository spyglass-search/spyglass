use shared::event::ClientInvoke;
use shared::response::LensResult;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::function_component;
use yew::prelude::*;

use crate::components::{icons, lens::LibraryLens, Header};
// use crate::listen;
use crate::utils::RequestState;
use crate::{install_lens, invoke};
// use shared::event::ClientEvent;
use shared::response::{InstallStatus, InstallableLens};

async fn fetch_available_lenses() -> Option<Vec<LensResult>> {
    match invoke(ClientInvoke::ListInstallableLenses.as_ref(), JsValue::NULL).await {
        Ok(results) => match serde_wasm_bindgen::from_value::<Vec<InstallableLens>>(results) {
            Ok(lenses) => {
                let parsed: Vec<LensResult> = lenses
                    .iter()
                    .map(|lens| LensResult {
                        author: lens.author.clone(),
                        name: lens.name.clone(),
                        label: lens.label(),
                        description: lens.description.clone(),
                        hash: lens.sha.clone(),
                        file_path: None,
                        html_url: Some(lens.html_url.clone()),
                        download_url: Some(lens.download_url.clone()),
                        progress: InstallStatus::default(),
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

pub struct DiscoverPage {
    req_available: RequestState,
    installable: Vec<LensResult>,
    filter_string: Option<String>,
    filter_input: NodeRef,
}

pub enum Msg {
    FetchAvailable,
    HandleFilter,
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
            filter_string: None,
            filter_input: NodeRef::default(),
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
            Msg::HandleFilter => {
                if let Some(el) = self.filter_input.cast::<HtmlInputElement>() {
                    let filter = el.value();
                    if filter.is_empty() {
                        self.filter_string = None;
                    } else {
                        self.filter_string = Some(filter.trim().to_string());
                    }

                    true
                } else {
                    false
                }
            }
            Msg::SetAvailable(results) => {
                if let Some(results) = results {
                    self.req_available = RequestState::Finished;
                    self.installable = results;
                    true
                } else {
                    self.req_available = RequestState::Error;
                    false
                }
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let contents = if self.req_available.is_done() {
            self.installable
                .iter()
                .filter_map(|data| {
                    if let Some(filter) = &self.filter_string {
                        if !data.name.to_lowercase().contains(filter)
                            && !data.description.to_lowercase().contains(filter)
                        {
                            return None;
                        }
                    }

                    Some(html! {<LibraryLens result={data.clone()} /> })
                })
                .collect::<Html>()
        } else {
            html! {
                <div class="flex justify-center">
                    <div class="p-16">
                        <icons::RefreshIcon width="w-16" height="h-16" animate_spin={true} />
                    </div>
                </div>
            }
        };

        let header_icon = html! { <icons::GlobeIcon classes="mr-2" height="h-5" width="h-5" /> };
        html! {
            <div>
                <Header label="Discover" icon={header_icon}/>
                <div class="flex flex-col gap-4 p-4">
                    <div>
                        <input type="text"
                            placeholder="filter lenses"
                            class="w-full rounded p-2 text-black text-sm"
                            onkeyup={link.callback(|_| Msg::HandleFilter)}
                            ref={self.filter_input.clone()}
                        />
                    </div>
                    {contents}
                </div>
            </div>
        }
    }
}
