use strum_macros::{Display, EnumString};
use yew::{classes, prelude::*, Children};
use yew_router::components::Link;

use crate::components::icons;
use crate::{pages, Route};

#[derive(Clone, EnumString, Display, PartialEq)]
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
                    (props.current == props.tab).then(|| Some("bg-stone-700")),
                    link_styles.clone()
                )
            }
            to={Route::SettingsPage { tab: props.tab.clone() }}
        >
            {props.children.clone()}
        </Link<Route>>
    }
}

#[derive(PartialEq, Properties)]
pub struct SettingsPageProps {
    pub tab: Tab,
}

#[function_component(SettingsPage)]
pub fn settings_page(props: &SettingsPageProps) -> Html {
    html! {
        <div class="text-white flex">
            <div class="flex-col h-screen w-64 bg-stone-900 p-4 top-0 left-0 z-40 sticky">
                <div class="mb-10">
                    <div class="uppercase mb-4 text-sm text-gray-500 font-bold">
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

                <div class="mb-10">
                    <div class="uppercase mb-4 text-sm text-gray-500 font-bold">
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
                    </ul>
                </div>
                <div class="mb-10">
                    <div class="uppercase mb-4 text-sm text-gray-500 font-bold">
                        {"Settings"}
                    </div>
                    <ul>
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
                    Tab::LensManager => html! { <pages::LensManagerPage /> },
                    Tab::PluginsManager => html! { <pages::PluginManagerPage /> },
                    Tab::Stats => html!{ <pages::StatsPage /> },
                    Tab::UserSettings => html! { <pages::UserSettingsPage /> },
                }
            }
            </div>
        </div>
    }
}
