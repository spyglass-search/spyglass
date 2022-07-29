use strum_macros::{Display, EnumString};
use yew::prelude::*;
use yew_router::components::Link;

use crate::{Route, pages};

#[derive(Clone, EnumString, Display, PartialEq)]
pub enum Tab {
    #[strum(serialize = "user")]
    UserSettings,
    #[strum(serialize = "lenses")]
    LensManager,
    #[strum(serialize = "plugins")]
    PluginsManager,
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
                <div class="mb-10 pr-3">
                    <span class="font-bold uppercase">{"Settings"}</span>
                    <ul>
                        <li>
                            <Link<Route> to={Route::SettingsPage { tab: Tab::LensManager }}>
                                {"Lens Manager"}
                            </Link<Route>>
                        </li>
                        <li>
                            <Link<Route> to={Route::SettingsPage { tab: Tab::PluginsManager }}>
                                {"Plugins Manager"}
                            </Link<Route>>
                        </li>
                        <li>
                            <Link<Route> to={Route::SettingsPage { tab: Tab::UserSettings }}>
                                {"User Settings"}
                            </Link<Route>>
                        </li>
                    </ul>
                </div>
            </div>
            <div class="flex-col flex-1">
            {
                match props.tab {
                    Tab::UserSettings => html! { <div>{"User Settings"}</div> },
                    Tab::LensManager => html! { <pages::LensManagerPage /> },
                    Tab::PluginsManager => html! { <pages::PluginManagerPage /> },
                }
            }
            </div>
        </div>
    }
}
