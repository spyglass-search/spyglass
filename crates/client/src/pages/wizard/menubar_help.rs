use crate::utils::{get_os, OsName};
use yew::prelude::*;

/// Confused about where the tray icon is?
#[function_component(MenubarHelpPage)]
pub fn menubar_help() -> Html {

    let os = get_os();

    let (example_img, menubar_name) = match os {
        OsName::Linux | OsName::MacOS | OsName::Unknown => ("macos-menubar-example.svg", "menubar"),
        OsName::Windows => ("windows-menubar-example.svg", "system tray"),
    };

    let click_str = match os {
        OsName::MacOS => "Left click",
        _ => "Right click"
    };

    html! {
        <div class="my-auto">
            <img src={example_img} alt="Location of the menubar menu" class="h-[128px] mx-auto my-6"/>
            <div class="font-bold text-lg">{format!("Spyglass lives in your {}.", menubar_name)}</div>
            <div class="text-sm text-neutral-400 px-8">
                {format!("{click_str} on the icon to access your library, discover new lenses, and adjust your settings.")}
            </div>
        </div>
    }
}
