use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::function_component;
use yew::prelude::*;

use crate::components::{btn::Btn, icons, Header};
use crate::invoke;
use crate::listen;
use crate::utils::RequestState;
use shared::event::ClientEvent;
use shared::event::ClientInvoke;
use shared::response::{LensResult, InstallStatus};

#[derive(Properties, PartialEq, Eq)]
pub struct LensProps {
    pub result: LensResult,
}

async fn fetch_user_installed_lenses() -> Option<Vec<LensResult>> {
    match invoke(ClientInvoke::ListInstalledLenses.as_ref(), JsValue::NULL).await {
        Ok(results) => match serde_wasm_bindgen::from_value(results) {
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

#[function_component(Lens)]
pub fn lens_component(props: &LensProps) -> Html {
    let component_styles = classes!(
        "rounded-md",
        "bg-neutral-700",
        "p-4",
        "text-white",
        "shadow-md",
        "overflow-hidden"
    );
    let result = &props.result;

    let detail_bar = match &result.progress {
        InstallStatus::Finished => {
            html! {
                <div class="mt-2 text-sm flex flex-row gap-2 items-center">
                    <a href="https://example.com" class="border-neutral-600 border cursor-pointer font-semibold px-2 py-1 rounded-md text-xs inline-block hover:bg-neutral-600">
                        {"Details"}
                    </a>
                    <a href="https://example.com" class="bg-red-700 cursor-pointer font-semibold px-2 py-1 rounded-md text-xs inline-block hover:bg-red-900">
                        {"Uninstall"}
                    </a>
                </div>
            }
        },
        InstallStatus::Installing { percent, status } => {
            html! {
                <div class="mt-2 text-sm">
                    <div class="text-xs pb-1">{status.clone()}</div>
                    <div class="w-full bg-stone-800 h-1 rounded-3xl text-xs">
                        <div class="bg-cyan-400 h-1 rounded-lg pl-2 flex items-center animate-pulse" style={format!("width: {percent}%")}></div>
                    </div>
              </div>
            }
        }
    };

    html! {
        <div class={component_styles}>
            <div class="mb-1">
                <div class="text-lg font-semibold">{result.title.to_string()}</div>
                <div class="text-sm text-neutral-400">
                    {"Crafted By:"}
                    <a href={format!("https://github.com/{}", result.author)} target="_blank" class="text-cyan-400">
                        {format!(" @{}", result.author)}
                    </a>
                </div>
            </div>
            <div class="text-sm text-neutral-400">{result.description.clone()}</div>
            {detail_bar}
        </div>
    }
}

pub struct LensManagerPage {
    lens_updater: RequestState,
    req_user_installed: RequestState,
    user_installed: Vec<LensResult>,
}
pub enum Msg {
    RunLensUpdate,
    RunOpenFolder,
    RunRefresher,
    UpdaterFinished,
    SetUserInstalled(Option<Vec<LensResult>>),
}

impl LensManagerPage {
    fn user_installed_tabview(&self) -> Html {
        if self.user_installed.is_empty() {
            html! {
                <div class="grid place-content-center h-48 w-full text-neutral-500">
                    <icons::EmojiSadIcon height="h-20" width="w-20" classes={classes!("mx-auto")}/>
                    <div class="mt-4">{"Install some lenses to get started!"}</div>
                </div>
            }
        } else {
            self.user_installed
                .iter()
                .map(|data| html! {<Lens result={data.clone()} /> })
                .collect::<Html>()
        }
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

        {
            let link = link.clone();
            spawn_local(async move {
                let cb = Closure::wrap(Box::new(move |_| {
                    link.send_message(Msg::UpdaterFinished);
                }) as Box<dyn Fn(JsValue)>);

                let _ = listen(ClientEvent::UpdateLensFinished.as_ref(), &cb).await;
                cb.forget();
            });
        }

        Self {
            lens_updater: RequestState::NotStarted,
            req_user_installed: RequestState::NotStarted,
            user_installed: Vec::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::RunOpenFolder => {
                spawn_local(async {
                    let _ = invoke(ClientInvoke::OpenLensFolder.as_ref(), JsValue::NULL).await;
                });

                false
            }
            Msg::RunLensUpdate => {
                self.lens_updater = RequestState::InProgress;
                spawn_local(async {
                    let _ = invoke(ClientInvoke::RunLensUpdater.as_ref(), JsValue::NULL).await;
                });

                true
            }
            Msg::RunRefresher => {
                // Don't run if requests are in flight.
                if self.req_user_installed == RequestState::InProgress {
                    return false;
                }

                self.req_user_installed = RequestState::InProgress;
                link.send_future(async {
                    Msg::SetUserInstalled(fetch_user_installed_lenses().await)
                });

                false
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
            Msg::UpdaterFinished => {
                self.lens_updater = RequestState::Finished;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let contents = if self.req_user_installed.is_done() {
            self.user_installed_tabview()
        } else {
            html! {
                <div class="flex justify-center">
                    <div class="p-16">
                        <icons::RefreshIcon width="w-16" height="h-16" animate_spin={true} />
                    </div>
                </div>
            }
        };

        let lens_update_icon = if self.lens_updater.in_progress() {
            html! { <icons::RefreshIcon width="w-3.5" height="h-3.5" animate_spin={true} /> }
        } else {
            html! { <icons::ArrowDownOnSquares width="w-3.5" height="h-3.5"  /> }
        };

        let header_icon = html! {
            <icons::CollectionIcon classes="mr-2" height="h-4" width="h-4" />
        };

        html! {
            <div>
                <Header label="My Library" icon={header_icon}>
                    <Btn onclick={link.callback(|_| Msg::RunOpenFolder)}>
                        <icons::FolderOpenIcon width="w-3.5" height="h-3.5" />
                        <div class="ml-2">{"Lens folder"}</div>
                    </Btn>
                    <Btn onclick={link.callback(|_| Msg::RunLensUpdate)} disabled={self.lens_updater.in_progress()}>
                        {lens_update_icon}
                        <div class="ml-2">{"Update"}</div>
                    </Btn>
                </Header>
                <div class="flex flex-col gap-4 p-4">{contents}</div>
            </div>
        }
    }
}
