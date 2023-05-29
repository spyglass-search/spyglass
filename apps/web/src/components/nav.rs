use ui_components::btn::{Btn, BtnSize, BtnType};
use ui_components::icons;
use yew::{platform::spawn_local, prelude::*};

use crate::metrics::{Metrics, WebClientEvent};
use crate::{auth0_login, auth0_logout, AuthStatus};

#[derive(Properties, PartialEq)]
pub struct NavBarProps {
    pub current_lens: Option<String>,
    pub session_uuid: String,
}

#[function_component(NavBar)]
pub fn nav_bar_component(props: &NavBarProps) -> Html {
    let auth_status = use_context::<AuthStatus>().expect("Ctxt not set up");
    let toggle_nav = use_state(|| false);
    let metrics = Metrics::new(false);
    let uuid = props.session_uuid.clone();

    let metrics_client = metrics.clone();
    let auth_login = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        let metrics = metrics_client.clone();
        let uuid = uuid.clone();
        spawn_local(async move {
            metrics.track(WebClientEvent::Login, &uuid).await;
            let _ = auth0_login().await;
        });
    });

    let uuid = props.session_uuid.clone();
    let auth_logout = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        let metrics = metrics.clone();
        let uuid = uuid.clone();
        spawn_local(async move {
            metrics.track(WebClientEvent::Logout, &uuid).await;
            let _ = auth0_logout().await;
        });
    });

    #[cfg(debug_assertions)]
    let debug_vars = html! {
        <>
            <div>
                <span class="text-cyan-700 font-bold">{"SPYGLASS_BACKEND: "}</span>
                <span>{dotenv!("SPYGLASS_BACKEND_DEV")}</span>
            </div>
            <div>
                <span class="text-cyan-700 font-bold">{"AUTH0_AUDIENCE: "}</span>
                <span>{dotenv!("AUTH0_AUDIENCE")}</span>
            </div>
            <div>
                <span class="text-cyan-700 font-bold">{"AUTH0_REDIRECT_URI: "}</span>
                <span>{dotenv!("AUTH0_REDIRECT_URI")}</span>
            </div>
        </>
    };

    #[cfg(not(debug_assertions))]
    let debug_vars = html! {};

    let toggle_nav_cb = {
        let toggle_nav_state = toggle_nav.clone();
        Callback::from(move |_| {
            let new_state = !(*toggle_nav_state);
            toggle_nav_state.set(new_state);
        })
    };

    html! {
        <>
            <div class="block w-full sm:hidden text-white bg-neutral-900 p-4">
                 <button onclick={toggle_nav_cb} class="flex items-center px-3 py-2 border rounded text-cyan-500 border-cyan-500 hover:text-white hover:border-white">
                    <svg class="fill-current h-3 w-3" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                        <title>{"Menu"}</title>
                        <path d="M0 3h20v2H0V3zm0 6h20v2H0V9zm0 6h20v2H0v-2z"/>
                    </svg>
                </button>
                { if *toggle_nav {
                    html! {
                        <div class="w-full block flex-grow lg:flex lg:items-center lg:w-auto pt-4">
                        {if auth_status.is_authenticated {
                            html! {
                                <div>
                                    <a href="/" class="p-2 flex flex-row text-lg items-center gap-2 rounded hover:bg-neutral-500">
                                        <icons::HomeIcon />
                                        <span>{"Home"}</span>
                                    </a>
                                </div>
                            }
                        } else {
                            html! {
                                <Btn size={BtnSize::Sm} _type={BtnType::Primary} onclick={auth_login.clone()} classes="w-full">
                                    {"Sign In"}
                                </Btn>
                            }
                        }}
                        </div>
                    }
                } else { html! {} }}
            </div>
            <div class="text-white hidden sm:block w-48 xl:w-64 min-w-max bg-stone-900 p-4 top-0 left-0 z-40 sticky h-screen">
                <a href="/" class="cursor-pointer"><img src="/icons/logo@2x.png" class="w-12 h-12 mx-auto" /></a>
                <div class="pt-4">
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
                <hr class="border border-neutral-700 mt-6 mb-4" />
                <div>
                    <a href="/" class="p-2 flex flex-row text-lg items-center gap-2 rounded hover:bg-neutral-500">
                        <icons::HomeIcon />
                        <span>{"Home"}</span>
                    </a>
                </div>
                <div class="mt-4">
                    <div class="uppercase mb-2 text-xs text-gray-500 font-bold">
                        {"My Q&As"}
                    </div>
                </div>
                <div class="absolute text-xs text-neutral-600 bottom-0 py-4 flex flex-col">
                    <div>
                        <span class="font-bold text-cyan-700">{"build: "}</span>
                        <span>{dotenv!("GIT_HASH")}</span>
                    </div>
                    {debug_vars}
                </div>
            </div>
        </>
    }
}
