use crate::components::{
    btn::{Btn, BtnSize, BtnType},
    icons,
};
use shared::response::{InstallStatus, LensResult};
use yew::function_component;
use yew::prelude::*;

#[derive(PartialEq, Eq)]
pub enum LensEvent {
    ShowDetails { name: String },
    Install { name: String },
    Uninstall { name: String },
}

#[derive(Properties, PartialEq)]
pub struct LensProps {
    pub result: LensResult,
    #[prop_or_default]
    pub onclick: Callback<LensEvent>,
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

    let lens_name = result.title.clone();
    let onclick = props.onclick.clone();

    let detail_bar = match &result.progress {
        InstallStatus::NotInstalled => {
            let install_cb = Callback::from(move |_| {
                onclick.emit(LensEvent::Install {
                    name: lens_name.clone(),
                })
            });
            html! {
                <div class="mt-2 text-sm flex flex-row gap-2 items-center">
                    <Btn _type={BtnType::Success} size={BtnSize::Xs} onclick={install_cb}>
                        <icons::DocumentDownloadIcon width="w-3.5" height="h-3.5" />
                        {"Install"}
                    </Btn>
                </div>
            }
        }
        InstallStatus::Finished => {
            let name = lens_name.clone();
            let show_onclick = onclick.clone();
            let show_cb = Callback::from(move |_| {
                show_onclick.emit(LensEvent::ShowDetails { name: name.clone() })
            });

            let uninstall_cb = Callback::from(move |_| {
                onclick.emit(LensEvent::Uninstall {
                    name: lens_name.clone(),
                })
            });
            html! {
                <div class="mt-2 text-sm flex flex-row gap-2 items-center">
                    <Btn size={BtnSize::Xs} onclick={show_cb}>{"Details"}</Btn>
                    <Btn _type={BtnType::Danger} size={BtnSize::Xs} onclick={uninstall_cb}>{"Uninstall"}</Btn>
                </div>
            }
        }
        InstallStatus::Installing { percent, status } => {
            html! {
                <div class="mt-2 text-sm">
                    <div class="text-xs pb-1">{status.clone()}</div>
                    <div class="w-full bg-stone-800 h-1 rounded-3xl text-xs">
                        <div class="bg-cyan-400 h-1 rounded-lg pl-2 flex items-center animate-pulse" style={format!("width: {percent}%")}></div>
                    </div>
              </div>
            }
        }
    };

    html! {
        <div class={component_styles}>
            <div class="mb-1">
                <div class="text-lg font-semibold">{result.title.to_string()}</div>
                <div class="text-sm text-neutral-400">
                    {"Crafted By:"}
                    <a href={format!("https://github.com/{}", result.author)} target="_blank" class="text-cyan-400">
                        {format!(" @{}", result.author)}
                    </a>
                </div>
            </div>
            <div class="text-sm text-neutral-400">{result.description.clone()}</div>
            {detail_bar}
        </div>
    }
}
