use strum_macros::{Display, EnumString};
use ui_components::icons;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::{classes, prelude::*, Children};
use yew_router::components::Link;
use yew_router::hooks::use_navigator;

use crate::{listen, pages, Route};
use shared::event::{ClientEvent, ListenPayload};

#[derive(Clone, EnumString, Display, PartialEq, Eq)]
pub enum Tab {
    #[strum(serialize = "connections")]
    ConnectionsManager,
    #[strum(serialize = "discover")]
    Discover,
    #[strum(serialize = "library")]
    LensManager,
    #[strum(serialize = "plugins")]
    PluginsManager,
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
        "hover:bg-neutral-700",
        "items-center",
        "p-2",
        "rounded",
        "w-full",
    );

    html! {
        <Link<Route>
            classes={
                classes!(
                    (props.current == props.tab).then_some(Some("bg-neutral-700")),
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
    let history = use_navigator().expect("History not available in this browser");

    spawn_local(async move {
        let cb = Closure::wrap(Box::new(move |payload: JsValue| {
            if let Ok(payload) = serde_wasm_bindgen::from_value::<ListenPayload<String>>(payload) {
                match payload.payload.as_str() {
                    "/settings/discover" => history.push(&Route::SettingsPage {
                        tab: pages::Tab::Discover,
                    }),
                    "/settings/library" => history.push(&Route::SettingsPage {
                        tab: pages::Tab::LensManager,
                    }),
                    "/settings/connections" => history.push(&Route::SettingsPage {
                        tab: pages::Tab::ConnectionsManager,
                    }),
                    "/settings/plugins" => history.push(&Route::SettingsPage {
                        tab: pages::Tab::PluginsManager,
                    }),
                    "/settings/user" => history.push(&Route::SettingsPage {
                        tab: pages::Tab::UserSettings,
                    }),
                    _ => history.push(&Route::SettingsPage {
                        tab: pages::Tab::LensManager,
                    }),
                }
            }
        }) as Box<dyn Fn(JsValue)>);
        let _ = listen(ClientEvent::Navigate.as_ref(), &cb).await;
        cb.forget();
    });

    html! {
        <div class="text-white flex h-screen">
            <div class="flex-col w-48 min-w-max bg-stone-900 p-4 top-0 left-0 z-40 sticky h-screen">
                <div class="mb-6">
                    <div class="uppercase mb-2 text-xs text-gray-500 font-bold">
                        {"Spyglass"}
                    </div>
                    <ul>
                        <li class="mb-2">
                            <NavLink tab={Tab::Discover} current={props.tab.clone()}>
                                <icons::GlobeIcon classes="mr-2" height="h-4" width="h-4" />
                                {"Discover"}
                            </NavLink>
                        </li>
                        <li class="mb-2">
                            <NavLink tab={Tab::LensManager} current={props.tab.clone()}>
                                <icons::CollectionIcon classes="mr-2" height="h-4" width="h-4" />
                                {"My Library"}
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
                            <NavLink tab={Tab::ConnectionsManager} current={props.tab.clone()}>
                                <icons::ShareIcon classes="mr-2" height="h-4" width="h-4" />
                                {"Connections"}
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
            <div class="flex-col flex-1 h-screen overflow-y-auto bg-neutral-800">
            {
                match props.tab {
                    #[allow(clippy::let_unit_value)]
                    Tab::ConnectionsManager => html! { <pages::ConnectionsManagerPage /> },
                    #[allow(clippy::let_unit_value)]
                    Tab::Discover => html! { <pages::DiscoverPage /> },
                    #[allow(clippy::let_unit_value)]
                    Tab::LensManager => html! { <pages::LensManagerPage /> },
                    #[allow(clippy::let_unit_value)]
                    Tab::PluginsManager => html! { <pages::PluginManagerPage /> },
                    #[allow(clippy::let_unit_value)]
                    Tab::UserSettings => html! { <pages::UserSettingsPage /> },
                }
            }
            </div>
        </div>
    }
}
