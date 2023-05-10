use ui_components::btn::{Btn, BtnSize, BtnType};
use ui_components::icons;
use yew::{platform::spawn_local, prelude::*};
use yew_router::prelude::use_navigator;

use super::LensList;
use crate::client::Lens;
use crate::{auth0_login, auth0_logout, AuthStatus, Route};

#[derive(Properties, PartialEq)]
pub struct NavBarProps {
    pub current_lens: Option<String>,
    #[prop_or_default]
    pub on_create_lens: Callback<Lens>,
    #[prop_or_default]
    pub on_select_lens: Callback<Lens>,
    #[prop_or_default]
    pub on_edit_lens: Callback<Lens>,
}

#[function_component(NavBar)]
pub fn nav_bar_component(props: &NavBarProps) -> Html {
    let navigator = use_navigator().expect("Navigator not available");
    let auth_status = use_context::<AuthStatus>().expect("Ctxt not set up");
    let user_data = auth_status.user_data.clone();

    let auth_login = Callback::from(|e: MouseEvent| {
        e.prevent_default();
        spawn_local(async {
            let _ = auth0_login().await;
        });
    });

    let auth_logout = Callback::from(|e: MouseEvent| {
        e.prevent_default();
        spawn_local(async {
            let _ = auth0_logout().await;
        });
    });

    let auth_status_handle = auth_status.clone();
    let on_create = props.on_create_lens.clone();
    let create_lens_cb = Callback::from(move |_| {
        let navigator = navigator.clone();
        let auth_status_handle: AuthStatus = auth_status_handle.clone();
        let on_create = on_create.clone();
        spawn_local(async move {
            // create a new lens
            let api = auth_status_handle.get_client();
            match api.lens_create().await {
                Ok(new_lens) => {
                    on_create.emit(new_lens.clone());
                    navigator.push(&Route::Edit {
                        lens: new_lens.name,
                    })
                }
                Err(err) => log::error!("error creating lens: {err}"),
            }
        });
    });

    #[cfg(debug_assertions)]
    let debug_vars = html! {
        <>
            <div><span class="text-cyan-700 font-bold">{"AUTH0_AUDIENCE: "}</span><span>{dotenv!("AUTH0_AUDIENCE")}</span></div>
            <div><span class="text-cyan-700 font-bold">{"AUTH0_REDIRECT_URI: "}</span><span>{dotenv!("AUTH0_REDIRECT_URI")}</span></div>
        </>
    };

    #[cfg(not(debug_assertions))]
    let debug_vars = html! {};

    html! {
        <div class="flex-col sm:w-32 md:w-48 xl:w-64 min-w-max bg-stone-900 p-4 top-0 left-0 z-40 sticky h-screen">
            <div class="mb-6">
                <div class="uppercase mb-2 text-xs text-gray-500 font-bold">
                    {"Spyglass"}
                </div>
                {if auth_status.is_authenticated {
                    if let Some(profile) = auth_status.user_profile {
                        html! {
                            <div class="mb-4 flex flex-col gap-4">
                                <div class="text-sm flex flex-row items-center gap-2">
                                    <img src={profile.picture} class="flex-none w-6 h-6 rounded-full mx-auto" />
                                    <div class="flex-grow">{profile.name}</div>
                                </div>
                                <Btn size={BtnSize::Sm} _type={BtnType::Primary} onclick={auth_logout} classes="w-full">
                                    {"Logout"}
                                </Btn>
                            </div>
                        }
                    } else {
                        html !{}
                    }
                } else {
                    html! {
                        <Btn size={BtnSize::Sm} _type={BtnType::Primary} onclick={auth_login} classes="w-full">
                            {"Sign In"}
                        </Btn>
                    }
                }}
            </div>
            <div class="mb-6">
                <div class="uppercase mb-2 text-xs text-gray-500 font-bold">
                    {"My Lenses"}
                </div>
                {if auth_status.is_authenticated {
                    html! {
                        <Btn size={BtnSize::Sm} classes="mb-2 w-full" onclick={create_lens_cb.clone()}>
                            <icons::PlusIcon width="w-4" height="h-4" />
                            <span>{"Create Lens"}</span>
                        </Btn>
                    }
                } else { html! {} }}
                {if let Some(user_data) = &user_data {
                    html!{
                        <LensList
                            current={props.current_lens.clone()}
                            lenses={user_data.lenses.clone()}
                            on_select={props.on_select_lens.clone()}
                            on_edit={props.on_edit_lens.clone()}
                        />
                    }
                } else {
                    html! {}
                }}
            </div>
            <div class="hidden">
                <div class="uppercase mb-2 text-xs text-gray-500 font-bold">
                    {"Searches"}
                </div>
                <ul>
                    <li class="mb-2">
                        <icons::GlobeIcon classes="mr-2" height="h-4" width="h-4" />
                        {"Search"}
                    </li>
                </ul>
            </div>
            <div class="absolute text-xs text-neutral-600 bottom-0 py-4 flex flex-col">
                <div>
                    <span class="font-bold text-cyan-700">{"build: "}</span>
                    <span>{dotenv!("GIT_HASH")}</span>
                </div>
                {debug_vars}
            </div>
        </div>
    }
}
