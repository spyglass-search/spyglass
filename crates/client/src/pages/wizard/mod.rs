use yew::prelude::*;

use crate::components::icons;
use shared::constants;

mod display_searchbar;
mod menubar_help;
enum WizardStage {
    MenubarHelp,
    DisplaySearchbarHelp,
    GetStarted,
    Done,
}

#[derive(Properties, PartialEq)]
pub struct WizardPageProps {
    #[prop_or_default]
    on_cancel: Callback<MouseEvent>,
    #[prop_or_default]
    on_next: Callback<MouseEvent>,
}

#[function_component(GetStartedPage)]
pub fn get_started_page(_props: &WizardPageProps) -> Html {
    let doc_styles = classes!(
        "bg-neutral-900",
        "border-neutral-700",
        "hover:bg-blue-900",
        "p-4",
        "rounded-lg",
        "text-center",
    );

    html! {
        <div class="p-4 bg-neutral-800 h-screen">
            <h1 class="text-2xl mb-4 flex flex-row gap-4">
                <div class="animate-wiggle-short hover:animate-wiggle">{"👋"}</div>
                <div>{"Let's Get Started"}</div>
            </h1>
            <div class="text-sm">
                <div class="flex flex-col gap-4 mb-4">
                    <a href="https://docs.spyglass.fyi/usage/indexing/local-files.html" target="_blank" class={doc_styles.clone()}>
                        <div class="flex flex-row items-center">
                            <icons::DesktopComputerIcon height="h-8" width="w-8" />
                            <div class="ml-2">{"Index local files"}</div>
                            <icons::ChevronRightIcon height="h-8" width="w-8" classes="ml-auto"/>
                        </div>
                    </a>
                    <a href="https://docs.spyglass.fyi/usage/indexing/bookmarks.html" target="_blank" class={doc_styles.clone()}>
                        <div class="flex flex-row items-center">
                            <icons::BookmarkIcon height="h-8" width="w-8" />
                            <div class="ml-2">{"Index your browser bookmarks & history"}</div>
                            <icons::ChevronRightIcon height="h-8" width="w-8" classes="ml-auto"/>
                        </div>
                    </a>
                    <a href="https://docs.spyglass.fyi/usage/indexing/web.html" target="_blank" class={doc_styles.clone()}>
                        <div class="flex flex-row items-center">
                            <icons::GlobeIcon height="h-8" width="w-8" />
                            <div class="ml-2">{"Index internet topics & sites"}</div>
                            <icons::ChevronRightIcon height="h-8" width="w-8" classes="ml-auto"/>
                        </div>
                    </a>
                </div>
                <div class="grid grid-cols-3 gap-2 text-sm">
                    <a href={constants::GITHUB_REPO_URL} target="_blank" class="text-center bg-neutral-900 rounded-lg border-neutral-700 p-4 hover:bg-amber-700">
                        <icons::StarIcon height="h-8" width="w-8" classes="mx-auto mb-2" />
                        <div>{"Star on GitHub"}</div>
                    </a>
                    <a href={constants::DISCORD_JOIN_URL} target="_blank" class="block text-center bg-neutral-900 rounded-lg p-4 hover:bg-indigo-900">
                        <img src="discord-logo.png" alt="Discord Logo" class="h-8 mx-auto mb-2"/>
                        <div>{"Join our Discord"}</div>
                    </a>
                    <a href={constants::PAYMENT_URL} target="_blank" class="text-center bg-neutral-900 rounded-lg border-neutral-700 p-4 hover:bg-green-900">
                        <icons::CurrencyIcon height="h-8" width="w-8" classes="mx-auto mb-2" />
                        <div>{"Support Us"}</div>
                    </a>
                </div>
            </div>
        </div>
    }
}

#[function_component(WizardPage)]
pub fn wizard_page() -> Html {
    let stage = use_state(|| WizardStage::MenubarHelp);

    let stage_clone = stage.clone();
    let handle_next = Callback::from(move |_| {
        let next_stage = match *stage_clone {
            WizardStage::MenubarHelp => WizardStage::DisplaySearchbarHelp,
            WizardStage::DisplaySearchbarHelp => WizardStage::GetStarted,
            WizardStage::GetStarted => WizardStage::Done,
            WizardStage::Done => WizardStage::Done,
        };

        stage_clone.set(next_stage);
    });

    match *stage {
        WizardStage::MenubarHelp => html!{ <menubar_help::MenubarHelpPage on_next={handle_next.clone()} /> },
        WizardStage::DisplaySearchbarHelp => html!{ <display_searchbar::DisplaySearchbarPage on_next={handle_next.clone()} /> },
        WizardStage::GetStarted => html! { <GetStartedPage on_next={handle_next} /> },
        WizardStage::Done => html!{},
    }
}
