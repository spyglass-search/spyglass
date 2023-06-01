use ui_components::btn::{Btn, BtnSize, BtnType};
use ui_components::icons;
use yew::{platform::spawn_local, prelude::*};
use yew_router::prelude::use_navigator;

use crate::metrics::{Metrics, WebClientEvent};
use crate::{auth0_login, auth0_logout, AuthStatus, Route};

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
    let navigator = use_navigator().unwrap();

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

    let mut history_buttons: Vec<Html> = Vec::new();
    if auth_status.is_authenticated {
        if let Some(user_data) = &auth_status.user_data {
            for history in &user_data.history {
                if history.lenses.len() == 1 && !history.qna.is_empty() {
                    let title = history.qna.get(0).unwrap().question.clone();
                    let lens = history.lenses.get(0).unwrap().clone();
                    let session_id = history.session_id.clone();

                    let nav = navigator.clone();
                    let session = session_id.clone();
                    let onclick = Callback::from(move |_| {
                        nav.push(&Route::SearchSession {
                            lens: lens.clone(),
                            chat_session: session.clone(),
                        })
                    });
                    history_buttons.push(html! {
                        <button key={session_id} {onclick} class="p-2 w-full text-left flex flex-row items-center gap-2 rounded hover:bg-neutral-500 overflow-clip group text-base">
                            <icons::ChatBubbleLeftRight width="w-4" height="h-4" />
                            <div class="flex-1 text-ellipsis max-h-6 overflow-hidden break-all relative">
                              {title}
                              <div class="absolute inset-y-0 right-0 w-8 z-10 bg-gradient-to-l from-neutral-900 group-hover:from-neutral-500"></div>
                            </div>
                        </button>
                    });
                }
            }
        }
    }

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
                            <a href="/" class="p-2 flex flex-row text-lg items-center gap-2 rounded hover:bg-neutral-500">
                                <icons::HomeIcon />
                                <span>{"Discover"}</span>
                            </a>
                        {if auth_status.is_authenticated {
                            html! {
                                <a href="/" class="p-2 flex flex-row text-lg items-center gap-2 rounded hover:bg-neutral-500">
                                    <icons::HomeIcon />
                                    <span>{"Home"}</span>
                                </a>
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
                <div class="flex flex-col gap-2">
                    <a href="/discover" class="p-2 flex flex-row text-lg items-center gap-2 rounded hover:bg-neutral-500">
                        <icons::GlobeIcon />
                        <span>{"Discover"}</span>
                    </a>
                    <a href="/" class="p-2 flex flex-row text-lg items-center gap-2 rounded hover:bg-neutral-500">
                        <icons::HomeIcon />
                        <span>{"Home"}</span>
                    </a>
                </div>
                <div class="mt-4">
                    <div class="uppercase mb-2 text-xs text-gray-500 font-bold">
                        {"My Q&As"}
                    </div>
                    {history_buttons}
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
