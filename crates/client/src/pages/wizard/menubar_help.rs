use yew::prelude::*;
use crate::components::btn;
use super::WizardPageProps;

/// Confused about where the tray icon is?
#[function_component(MenubarHelpPage)]
pub fn menubar_help(props: &WizardPageProps) -> Html {
    let example_img = if cfg!(target_os = "macos") {
        "macos-menubar-example.svg"
    } else if cfg!(target_os = "windows") {
        "windows-menubar-example.svg"
    } else {
        "macos-menubar-example.svg"
    };

    html! {
        <div class="py-4 px-8 bg-neutral-800 h-screen text-center flex flex-col gap-4">
            <img src={example_img} alt="Location of the menubar menu" class="h-[128px] mx-auto my-6"/>
            <div class="font-bold text-lg">{"Spyglass lives in your menubar"}</div>
            <div class="text-sm text-neutral-400 px-8">
                {"Click on the menubar icon to access your library, discover new lenses, and adjust your settings."}
            </div>
            <div class="mt-auto mb-4 flex flex-col gap-4">
                <btn::Btn onclick={props.on_next.clone()}>
                    {"Showing & Hiding the Search Bar"}
                </btn::Btn>
                <btn::Btn _type={btn::BtnType::Danger} onclick={props.on_cancel.clone()}>
                    {"Stop the wizard, I'm a seasoned expert."}
                </btn::Btn>
            </div>
        </div>
    }
}
