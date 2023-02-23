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
struct LensActionBarProps {
    pub result: LensResult,
    #[prop_or_default]
    pub onclick: Callback<LensEvent>,
    #[prop_or_default]
    pub in_progress: bool,
}

#[function_component(LensActionBar)]
fn lens_action_bar(props: &LensActionBarProps) -> Html {
    let navigator = use_navigator().unwrap();
    let result = &props.result;
    let lens_name = result.name.clone();
    let onclick = props.onclick.clone();

    let icon_h = "h-3.5";
    let icon_w = "w-3.5";

    match &result.progress {
        InstallStatus::NotInstalled => {
            let lens_display_name = lens_name.clone();
            let install_cb = Callback::from(move |_| {
                onclick.emit(LensEvent::Install {
                    name: lens_display_name.clone(),
                })
            });
            html! {
                <div class="mt-2 text-sm flex flex-row gap-2 items-center">
                    <Btn href={view_link(&lens_name.clone())} size={BtnSize::Xs}>
                        <icons::EyeIcon width={icon_w} height={icon_h} />
                        {"Details"}
                    </Btn>
                    <Btn _type={BtnType::Success} size={BtnSize::Xs} onclick={install_cb} disabled={props.in_progress}>
                        {if props.in_progress {
                            html! { <icons::RefreshIcon animate_spin={true} width={icon_w} height={icon_h} /> }
                        } else {
                            html!{ <icons::DocumentDownloadIcon width={icon_w} height={icon_h} /> }
                        }}
                        {"Install"}
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

            let view_btn = {
                let label = html! {
                    <>
                        <icons::EyeIcon width={icon_w} height={icon_h} />
                        {"Details"}
                    </>
                };
                match result.lens_type {
                    LensType::Lens => {
                        html! { <Btn href={view_link(&lens_name.clone())} size={BtnSize::Xs}>{label}</Btn> }
                    }
                    LensType::Plugin => {
                        let onclick = Callback::from(move |_| {
                            navigator.push(&Route::SettingsPage {
                                tab: pages::Tab::PluginsManager,
                            })
                        });
                        html! { <Btn {onclick} size={BtnSize::Xs}>{label}</Btn> }
                    }
                    LensType::API => {
                        let onclick = Callback::from(move |_| {
                            navigator.push(&Route::SettingsPage {
                                tab: pages::Tab::ConnectionsManager,
                            })
                        });
                        html! { <Btn {onclick} size={BtnSize::Xs}>{label}</Btn> }
                    }
                    LensType::Internal => {
                        let onclick = Callback::from(move |_| {
                            navigator.push(&Route::SettingsPage {
                                tab: pages::Tab::UserSettings,
                            })
                        });
                        html! { <Btn {onclick} size={BtnSize::Xs}>{label}</Btn> }
                    }
                }
            };

            let uninstall_btn = match result.lens_type {
                LensType::Lens => html! {
                    <Btn _type={BtnType::Danger} size={BtnSize::Xs} onclick={uninstall_cb} disabled={props.in_progress}>
                        {if props.in_progress {
                            html! { <icons::RefreshIcon animate_spin={true} width={icon_w} height={icon_h} /> }
                        } else {
                            html!{ <icons::TrashIcon width={icon_w} height={icon_h} /> }
                        }}
                    {"Uninstall"}
                    </Btn>
                },
                _ => html! {},
            };

            html! {
                <div class="mt-2 text-sm flex flex-row gap-2 items-center">
                    {view_btn}
                    {uninstall_btn}
                    <div class="ml-auto text-neutral-200">{format!("{buf} docs")}</div>
                </div>
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
    }
}

/// Create a view link to the lens directory HTML page.
fn view_link(lens_name: &str) -> String {
    format!(
        "https://lenses.spyglass.fyi/lenses/{}",
        lens_name.clone().to_lowercase().replace('_', "-")
    )
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
    let component_styles = classes!(
        "rounded-md",
        "bg-neutral-700",
        "p-4",
        "text-white",
        "shadow-md",
        "overflow-hidden"
    );

    let result = &props.result;

    html! {
        <div class={component_styles}>
            <div class="mb-1">
                <div class="text-base font-semibold">{result.label.to_string()}</div>
                <div class="text-sm text-neutral-400">
                    {"Crafted By:"}
                    <a href={format!("https://github.com/{}", result.author)} target="_blank" class="text-cyan-400">
                        {format!(" @{}", result.author)}
                    </a>
                </div>
            </div>
            <div class="text-sm text-neutral-300">{result.description.clone()}</div>
            <LensActionBar result={props.result.clone()} onclick={props.onclick.clone()} in_progress={props.in_progress} />
        </div>
    }
}
