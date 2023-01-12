use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::{
    btn::Btn,
    icons,
    lens::{LensEvent, LibraryLens},
    Header,
};
use crate::utils::RequestState;
use crate::{invoke, listen, tauri_invoke};
use shared::event::ClientInvoke;
use shared::event::{ClientEvent, UninstallLensParams};
use shared::response::LensResult;

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

pub struct LensManagerPage {
    lens_updater: RequestState,
    req_user_installed: RequestState,
    user_installed: Vec<LensResult>,
}
pub enum Msg {
    HandleLensEvent(LensEvent),
    RunLensUpdate,
    RunOpenFolder,
    RunRefresher,
    UpdaterFinished,
    SetUserInstalled(Option<Vec<LensResult>>),
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
            Msg::HandleLensEvent(event) => {
                spawn_local(async {
                    if let LensEvent::Uninstall { name } = event {
                        let _ = tauri_invoke::<_, ()>(
                            ClientInvoke::UninstallLens,
                            &UninstallLensParams { name },
                        )
                        .await;
                    }
                });

                false
            }
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
                    .map(|data| html! {<LibraryLens onclick={link.callback(Msg::HandleLensEvent)} result={data.clone()} /> })
                    .collect::<Html>()
            }
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
