use ui_components::icons;
use yew::{platform::spawn_local, prelude::*};
use yew_router::prelude::{use_navigator, Link};

use crate::{auth0_login, auth0_logout, AuthStatus, Route};
pub mod search;

#[derive(PartialEq, Properties)]
pub struct NavLinkProps {
    tab: Route,
    children: Children,
    current: Route,
}

#[function_component(NavLink)]
pub fn nav_link(props: &NavLinkProps) -> Html {
    let link_styles = classes!(
        "flex-row",
        "flex",
        "hover:bg-neutral-700",
        "items-center",
        "p-2",
        "rounded",
        "w-full",
        (props.current == props.tab).then_some(Some("bg-neutral-700"))
    );

    html! {
        <Link<Route> classes={link_styles} to={props.tab.clone()}>
            {props.children.clone()}
        </Link<Route>>
    }
}

#[derive(Properties, PartialEq)]
pub struct AppPageProps {
    pub lens: String,
}

#[function_component]
pub fn AppPage(props: &AppPageProps) -> Html {
    let navigator = use_navigator().unwrap();
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

    let mut lenses = Vec::new();
    if let Some(user_data) = user_data {
        for lens in user_data.lenses {
            let navi = navigator.clone();
            let lens_name = lens.name.clone();
            let onclick = Callback::from(move |_| {
                navi.push(&Route::Search {
                    lens: lens_name.clone(),
                })
            });
            lenses.push(html! {
                <li>
                    <a class="hover:bg-cyan-600 cursor-pointer flex flex-row items-center p-2 rounded" {onclick}>
                        <icons::CollectionIcon classes="mr-2" height="h-4" width="h-4" />
                        {lens.name.clone()}
                    </a>
                </li>
            });
        }
    }

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
                                    <button class="text-sm rounded-md border border-cyan-500 p-2" onclick={auth_logout}>
                                        {"Logout"}
                                    </button>
                                </div>
                            }
                        } else {
                            html !{}
                        }
                    } else {
                        html! {
                            <div class="mb-4 flex flex-col">
                                <button class="text-sm rounded-md border border-cyan-500 p-2" onclick={auth_login}>
                                    {"Signin"}
                                </button>
                            </div>
                        }
                    }}
                </div>
                <div class="mb-6">
                    <div class="uppercase mb-2 text-xs text-gray-500 font-bold">
                        {"My Collections"}
                    </div>
                    <ul>
                        {lenses}
                    </ul>
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
                <search::SearchPage lens={props.lens.clone()} />
            </div>
        </div>
    }
}
