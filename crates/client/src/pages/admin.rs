use serde::Deserialize;
use strum_macros::{Display, EnumString};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::{classes, prelude::*, Children};
use yew_router::components::Link;
use yew_router::history::History;
use yew_router::hooks::use_history;

use crate::components::icons;
use crate::{listen, pages, Route};
use shared::event::ClientEvent;

#[derive(Debug, Deserialize)]
struct ListenPayload {
    payload: String,
}

#[derive(Clone, EnumString, Display, PartialEq, Eq)]
pub enum Tab {
    #[strum(serialize = "lenses")]
    LensManager,
    #[strum(serialize = "plugins")]
    PluginsManager,
    #[strum(serialize = "stats")]
    Stats,
    #[strum(serialize = "user")]
    UserSettings,
}

#[derive(PartialEq, Properties)]
pub struct NavLinkProps {
    tab: Tab,
    children: Children,
    current: Tab,
}

#[function_component(NavLink)]
pub fn nav_link(props: &NavLinkProps) -> Html {
    let link_styles = classes!(
        "flex-row",
        "flex",
        "hover:bg-stone-700",
        "items-center",
        "p-2",
        "rounded",
        "w-full",
    );

    html! {
        <Link<Route>
            classes={
                classes!(
                    (props.current == props.tab).then_some(Some("bg-stone-700")),
                    link_styles
                )
            }
            to={Route::SettingsPage { tab: props.tab.clone() }}
        >
            {props.children.clone()}
        </Link<Route>>
    }
}

#[derive(PartialEq, Properties, Eq)]
pub struct SettingsPageProps {
    pub tab: Tab,
}

#[function_component(SettingsPage)]
pub fn settings_page(props: &SettingsPageProps) -> Html {
    let history = use_history().unwrap();

    spawn_local(async move {
        let cb = Closure::wrap(Box::new(move |payload: JsValue| {
            if let Ok(payload) = payload.into_serde::<ListenPayload>() {
                match payload.payload.as_str() {
                    "/settings/lenses" => history.push(Route::SettingsPage {
                        tab: pages::Tab::LensManager,
                    }),
                    "/settings/plugins" => history.push(Route::SettingsPage {
                        tab: pages::Tab::PluginsManager,
                    }),
                    "/settings/stats" => history.push(Route::SettingsPage {
                        tab: pages::Tab::Stats,
                    }),
                    "/settings/user" => history.push(Route::SettingsPage {
                        tab: pages::Tab::UserSettings,
                    }),
                    _ => history.push(Route::SettingsPage {
                        tab: pages::Tab::Stats,
                    }),
                }
            }
        }) as Box<dyn Fn(JsValue)>);
        let _ = listen(ClientEvent::Navigate.as_ref(), &cb).await;
        cb.forget();
    });

    html! {
        <div class="text-white flex">
            <div class="flex-col h-screen w-48 min-w-max bg-stone-900 p-4 top-0 left-0 z-40 sticky">
                <div class="mb-6">
                    <div class="uppercase mb-2 text-xs text-gray-500 font-bold">
                        {"Spyglass"}
                    </div>
                    <ul>
                        <li class="mb-2">
                            <NavLink tab={Tab::Stats} current={props.tab.clone()}>
                                <icons::ChartBarIcon classes="mr-2" height="h-4" width="h-4" />
                                {"Crawl Status"}
                            </NavLink>
                        </li>
                    </ul>
                </div>

                <div class="mb-6">
                    <div class="uppercase mb-2 text-xs text-gray-500 font-bold">
                        {"Configuration"}
                    </div>
                    <ul>
                        <li class="mb-2">
                            <NavLink tab={Tab::LensManager} current={props.tab.clone()}>
                                <icons::FilterIcon classes="mr-2" height="h-4" width="h-4" />
                                {"Lenses"}
                            </NavLink>
                        </li>
                        <li class="mb-2">
                            <NavLink tab={Tab::PluginsManager} current={props.tab.clone()}>
                                <icons::ChipIcon classes="mr-2" height="h-4" width="h-4" />
                                {"Plugins"}
                            </NavLink>
                        </li>
                        <li class="mb-2">
                            <NavLink tab={Tab::UserSettings} current={props.tab.clone()}>
                                <icons::AdjustmentsIcon classes="mr-2" height="h-4" width="h-4" />
                                {"User Settings"}
                            </NavLink>
                        </li>
                    </ul>
                </div>
            </div>
            <div class="flex-col flex-1">
            {
                match props.tab {
                    #[allow(clippy::let_unit_value)]
                    Tab::LensManager => html! { <pages::LensManagerPage /> },
                    #[allow(clippy::let_unit_value)]
                    Tab::PluginsManager => html! { <pages::PluginManagerPage /> },
                    #[allow(clippy::let_unit_value)]
                    Tab::Stats => html!{ <pages::StatsPage /> },
                    #[allow(clippy::let_unit_value)]
                    Tab::UserSettings => html! { <pages::UserSettingsPage /> },
                }
            }
            </div>
        </div>
    }
}
