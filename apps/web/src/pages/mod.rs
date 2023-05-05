use ui_components::{
    btn::{Btn, BtnSize, BtnType},
    icons,
};
use yew::{platform::spawn_local, prelude::*};
use yew_router::prelude::use_navigator;

use crate::{auth0_login, auth0_logout, client::Lens, AuthStatus, Route};

pub mod create;
pub mod search;

#[derive(Properties, PartialEq)]
pub struct AppPageProps {
    #[prop_or_default]
    pub current_lens: Option<String>,
    #[prop_or_default]
    pub children: Children,
}

#[function_component]
pub fn AppPage(props: &AppPageProps) -> Html {
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
    let create_lens_cb = Callback::from(move |_| {
        let navigator = navigator.clone();
        let auth_status_handle = auth_status_handle.clone();
        spawn_local(async move {
            // create a new lens
            let api = auth_status_handle.get_client();
            match api.lens_create().await {
                Ok(new_lens) => navigator.push(&Route::Edit {
                    lens: new_lens.name,
                }),
                Err(err) => log::error!("error creating lens: {err}"),
            }
        });
    });

    html! {
        <div class="text-white flex h-screen">
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
                        html!{ <LensList current={props.current_lens.clone()} lenses={user_data.lenses.clone()} /> }
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
            </div>
            <div class="flex-col flex-1 h-screen overflow-y-auto bg-neutral-800">
                {props.children.clone()}
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct LensListProps {
    current: Option<String>,
    lenses: Option<Vec<Lens>>,
}

#[function_component(LensList)]
pub fn lens_list(props: &LensListProps) -> Html {
    let navigator = use_navigator().unwrap();
    let default_classes = classes!(
        "hover:bg-cyan-600",
        "cursor-pointer",
        "flex",
        "flex-grow",
        "flex-row",
        "items-center",
        "py-1.5",
        "px-2",
        "rounded",
        "text-sm"
    );

    let current_lens = props.current.clone().unwrap_or_default();
    let mut html = Vec::new();
    let lenses = props.lenses.clone();
    for lens in lenses.unwrap_or_default() {
        let classes = classes!(
            default_classes.clone(),
            if current_lens == lens.name {
                Some("bg-cyan-800")
            } else {
                None
            }
        );

        let navi = navigator.clone();
        let lens_name = lens.name.clone();
        let onclick = Callback::from(move |_| {
            navi.push(&Route::Search {
                lens: lens_name.clone(),
            })
        });

        let icon = if lens.is_public {
            html! { <icons::GlobeIcon classes="mr-2" height="h-3" width="w-3" /> }
        } else {
            html! { <icons::CollectionIcon classes="mr-2" height="h-3" width="w-3" /> }
        };

        let navi = navigator.clone();
        let lens_name = lens.name.clone();
        let on_edit = Callback::from(move |e: MouseEvent| {
            e.stop_immediate_propagation();
            navi.push(&Route::Edit { lens: lens_name.clone() })
        });

        let edit_icon = if lens.is_public {
            html! {}
        } else {
            html! {
                <Btn size={BtnSize::Sm} _type={BtnType::Borderless} classes="ml-auto" onclick={on_edit}>
                    <icons::PencilIcon height="h-3" width="w-3" />
                </Btn>
            }
        };

        html.push(html! {
            <li class="mb-1 flex flex-row items-center">
                <a class={classes.clone()} {onclick}>
                    {icon}
                    {lens.display_name.clone()}
                    {edit_icon}
                </a>
            </li>
        });
    }

    html! { <ul>{html}</ul> }
}
