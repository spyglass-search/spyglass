use crate::{
    components::{
        btn::{Btn, BtnSize, BtnType},
        icons,
    },
    pages, Route,
};
use num_format::{Buffer, Locale};
use shared::response::{InstallStatus, LensResult, LensType};
use yew::function_component;
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(PartialEq, Eq)]
pub enum LensEvent {
    Install { name: String },
    Uninstall { name: String },
}

#[derive(Properties, PartialEq)]
pub struct LensProps {
    pub result: LensResult,
    #[prop_or_default]
    pub onclick: Callback<LensEvent>,
    #[prop_or_default]
    pub in_progress: bool,
}

#[function_component(LibraryLens)]
pub fn lens_component(props: &LensProps) -> Html {
    let navigator = use_navigator().unwrap();
    let component_styles = classes!(
        "rounded-md",
        "bg-neutral-700",
        "p-4",
        "text-white",
        "shadow-md",
        "overflow-hidden"
    );

    let result = &props.result;

    let lens_name = result.name.clone();
    let onclick = props.onclick.clone();

    let detail_bar = match &result.progress {
        InstallStatus::NotInstalled => {
            let lens_display_name = lens_name.clone();
            let install_cb = Callback::from(move |_| {
                onclick.emit(LensEvent::Install {
                    name: lens_display_name.clone(),
                })
            });
            html! {
                <div class="mt-2 text-sm flex flex-row gap-2 items-center">
                    <Btn _type={BtnType::Success} size={BtnSize::Xs} onclick={install_cb} disabled={props.in_progress}>
                        {if props.in_progress {
                            html! { <icons::RefreshIcon animate_spin={true} width="w-3.5" height="h-3.5" /> }
                        } else {
                            html!{ <icons::DocumentDownloadIcon width="w-3.5" height="h-3.5" /> }
                        }}
                        {"Install"}
                    </Btn>
                    <Btn href={format!("https://lenses.spyglass.fyi/lenses/{}", lens_name.clone().to_lowercase().replace('_', "-"))} size={BtnSize::Xs}>
                        <icons::LinkIcon width="w-3.5" height="h-3.5" />
                        {"View Details"}
                    </Btn>
                </div>
            }
        }
        InstallStatus::Finished { num_docs } => {
            let name = lens_name.clone();
            let uninstall_cb =
                Callback::from(move |_| onclick.emit(LensEvent::Uninstall { name: name.clone() }));

            let mut buf = Buffer::default();
            buf.write_formatted(num_docs, &Locale::en);

            match result.lens_type {
                LensType::Lens => {
                    html! {
                        <div class="mt-2 text-sm flex flex-row gap-2 items-center">
                            <Btn href={format!("https://lenses.spyglass.fyi/lenses/{}", lens_name.clone().to_lowercase().replace('_', "-"))} size={BtnSize::Xs}>
                                <icons::LinkIcon width="w-3.5" height="h-3.5" />
                                {"View Details"}
                            </Btn>
                            <Btn _type={BtnType::Danger} size={BtnSize::Xs} onclick={uninstall_cb} disabled={props.in_progress}>
                                {if props.in_progress {
                                    html! { <icons::RefreshIcon animate_spin={true} width="w-3.5" height="h-3.5" /> }
                                } else {
                                    html!{ <icons::TrashIcon width="w-3.5" height="h-3.5" /> }
                                }}
                                {"Uninstall"}
                            </Btn>
                            <div class="ml-auto text-neutral-200">{format!("{buf} docs")}</div>
                        </div>
                    }
                }
                LensType::Plugin => {
                    let onclick = Callback::from(move |_| {
                        navigator.push(&Route::SettingsPage {
                            tab: pages::Tab::PluginsManager,
                        })
                    });
                    html! {
                        <div class="mt-2 text-sm flex flex-row gap-2 items-center">
                            <Btn {onclick} size={BtnSize::Xs}>
                                <icons::LinkIcon width="w-3.5" height="h-3.5" />
                                {"View Details"}
                            </Btn>
                            <div class="ml-auto text-neutral-200">{format!("{buf} docs")}</div>
                        </div>
                    }
                }
                LensType::API => {
                    let onclick = Callback::from(move |_| {
                        navigator.push(&Route::SettingsPage {
                            tab: pages::Tab::ConnectionsManager,
                        })
                    });
                    html! {
                        <div class="mt-2 text-sm flex flex-row gap-2 items-center">
                            <Btn {onclick} size={BtnSize::Xs}>
                                <icons::LinkIcon width="w-3.5" height="h-3.5" />
                                {"View Details"}
                            </Btn>
                            <div class="ml-auto text-neutral-200">{format!("{buf} docs")}</div>
                        </div>
                    }
                }
                LensType::Internal => {
                    let onclick = Callback::from(move |_| {
                        navigator.push(&Route::SettingsPage {
                            tab: pages::Tab::UserSettings,
                        })
                    });
                    html! {
                        <div class="mt-2 text-sm flex flex-row gap-2 items-center">
                            <Btn {onclick} size={BtnSize::Xs}>
                                <icons::LinkIcon width="w-3.5" height="h-3.5" />
                                {"View Details"}
                            </Btn>
                            <div class="ml-auto text-neutral-200">{format!("{buf} docs")}</div>
                        </div>
                    }
                }
            }
        }
        InstallStatus::Installing { percent, status } => match result.lens_type {
            LensType::Lens | LensType::Internal => {
                html! {
                    <div class="mt-2 text-sm">
                        <div class="text-xs pb-1">{status.clone()}</div>
                        <div class="w-full bg-stone-800 h-1 rounded-3xl text-xs">
                            <div class="bg-cyan-400 h-1 rounded-lg pl-2 flex items-center animate-pulse" style={format!("width: {percent}%")}></div>
                        </div>
                    </div>
                }
            }
            _ => {
                html! {
                    <div class="mt-2 text-sm">
                        <div class="text-xs pb-1">{status.clone()}</div>
                        <div class="w-full bg-stone-800 h-1 rounded-3xl text-xs">
                            <div class="bg-cyan-400 h-1 rounded-lg pl-2 flex items-center animate-pulse" style={"width: 100%"}></div>
                        </div>
                    </div>
                }
            }
        },
    };

    html! {
        <div class={component_styles}>
            <div class="mb-1">
                <div class="text-lg font-semibold">{result.label.to_string()}</div>
                <div class="text-sm text-neutral-400">
                    {"Crafted By:"}
                    <a href={format!("https://github.com/{}", result.author)} target="_blank" class="text-cyan-400">
                        {format!(" @{}", result.author)}
                    </a>
                </div>
            </div>
            <div class="text-sm text-neutral-200">{result.description.clone()}</div>
            {detail_bar}
        </div>
    }
}
