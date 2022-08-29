use yew::prelude::*;

use crate::components::icons;

#[derive(Properties, PartialEq, Eq)]
pub struct StartupPageProps {
    #[prop_or_default]
    pub status_caption: String,
}

#[function_component(StartupPage)]
pub fn startup_page(props: &StartupPageProps) -> Html {
    html! {
        <div class="flex flex-col place-content-center place-items-center mt-14">
            <icons::RefreshIcon animate_spin={true} height="h-16" width="w-16" />
            <div class="mt-4 font-medium">{"Starting Spyglass"}</div>
            <div class="mt-1 text-stone-500 text-sm">{props.status_caption.clone()}</div>
        </div>
    }
}
