use shared::response::{InstallStatus, LensResult};
use yew::function_component;
use yew::prelude::*;

#[derive(Properties, PartialEq, Eq)]
pub struct LensProps {
    pub result: LensResult,
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

    let detail_bar = match &result.progress {
        InstallStatus::Finished => {
            html! {
                <div class="mt-2 text-sm flex flex-row gap-2 items-center">
                    <a href="https://example.com" class="border-neutral-600 border cursor-pointer font-semibold px-2 py-1 rounded-md text-xs inline-block hover:bg-neutral-600">
                        {"Details"}
                    </a>
                    <a href="https://example.com" class="bg-red-700 cursor-pointer font-semibold px-2 py-1 rounded-md text-xs inline-block hover:bg-red-900">
                        {"Uninstall"}
                    </a>
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
