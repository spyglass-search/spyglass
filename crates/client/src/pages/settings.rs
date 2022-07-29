use crate::pages;
use yew::prelude::*;

#[derive(Clone, PartialEq)]
enum Tab {
    UserSettings,
    LensManager,
    PluginsManager,
}

#[function_component(SettingsPage)]
pub fn settings_page() -> Html {
    let selected_tab = use_state_eq(|| Tab::UserSettings);

    let onclick = |tab: Tab| {
        let selected_tab = selected_tab.clone();
        Callback::from(move |_| {
            let tab = tab.clone();
            selected_tab.set(tab);
        })
    };

    html! {
        <div class="text-white flex">
            <div class="flex-col h-screen w-64 bg-stone-900 p-4 top-0 left-0 z-40 sticky">
                <div class="mb-10 pr-3">
                    <span class="font-bold uppercase">{"Settings"}</span>
                    <ul>
                        <li><button onclick={onclick(Tab::LensManager)}>{"Lens Manager"}</button></li>
                        <li><button onclick={onclick(Tab::PluginsManager)}>{"Plugin Manager"}</button></li>
                        <li><button onclick={onclick(Tab::UserSettings)}>{"User Settings"}</button></li>
                    </ul>
                </div>
            </div>
            <div class="flex-col flex-1">
            {
                match *selected_tab {
                    Tab::UserSettings => html! { <div>{"User Settings"}</div> },
                    Tab::LensManager => html! { <pages::LensManagerPage /> },
                    Tab::PluginsManager => html! { <pages::PluginManagerPage /> },
                }
            }
            </div>
        </div>
    }
}
