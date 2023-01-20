use strum_macros::{Display, EnumString};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::hooks::use_navigator;

use crate::components::{btn, forms::SettingChangeEvent, icons};
use crate::{tauri_invoke, Route};
use shared::event::{ClientInvoke, WizardFinishedParams};

mod display_searchbar;
mod indexing_help;
mod menubar_help;

#[derive(Clone, PartialEq, Eq, EnumString, Display)]
pub enum WizardStage {
    #[strum(serialize = "menubar")]
    MenubarHelp,
    #[strum(serialize = "shortcuts")]
    DisplaySearchbarHelp,
    #[strum(serialize = "index-cloud")]
    IndexCloud,
    #[strum(serialize = "index-files")]
    IndexFiles,
    #[strum(serialize = "index-web")]
    IndexWeb,
    #[strum(serialize = "done")]
    Done,
}

#[derive(Properties, PartialEq)]
pub struct WizardProps {
    pub stage: WizardStage,
}

#[function_component(WizardPage)]
pub fn wizard_page(props: &WizardProps) -> Html {
    let nav = use_navigator().expect("History not available in this browser");
    let toggle_file_indexer = use_state(|| false);

    let cur_stage = props.stage.clone();
    let nav_clone = nav.clone();
    let tfi_state = toggle_file_indexer.clone();
    let handle_next = Callback::from(move |_| {
        let next_stage = match cur_stage {
            WizardStage::MenubarHelp => WizardStage::DisplaySearchbarHelp,
            WizardStage::DisplaySearchbarHelp => WizardStage::IndexFiles,
            WizardStage::IndexFiles => WizardStage::IndexCloud,
            WizardStage::IndexCloud => WizardStage::IndexWeb,
            WizardStage::IndexWeb => WizardStage::Done,
            _ => WizardStage::Done,
        };

        if next_stage == WizardStage::Done {
            let params = WizardFinishedParams {
                toggle_file_indexer: *tfi_state,
            };

            spawn_local(async move {
                let _ = tauri_invoke::<_, ()>(ClientInvoke::WizardFinished, &params).await;
            });
            return;
        }

        nav_clone.push(&Route::Wizard { stage: next_stage });
    });

    let tfi_state = toggle_file_indexer;
    let handle_onchange = Callback::from(move |event: SettingChangeEvent| {
        if let Ok(new_value) = serde_json::from_str::<bool>(&event.new_value) {
            tfi_state.set(new_value);
        }
    });

    let mut next_label = String::new();
    let content = match props.stage {
        WizardStage::MenubarHelp => {
            next_label = "Show/hide the searchbar".into();
            html! { <menubar_help::MenubarHelpPage /> }
        }
        WizardStage::DisplaySearchbarHelp => {
            next_label = "Indexing files, web content, & more".into();
            html! { <display_searchbar::DisplaySearchbarPage /> }
        }
        WizardStage::IndexFiles => {
            next_label = "Indexing Cloud Accounts".into();
            html! { <indexing_help::IndexFilesHelp onchange={handle_onchange} /> }
        }
        WizardStage::IndexCloud => {
            next_label = "Indexing Web Content".into();
            html! { <indexing_help::IndexCloudHelp /> }
        }
        WizardStage::IndexWeb => {
            next_label = "Ready to go!".into();
            html! { <indexing_help::IndexWebHelp /> }
        }
        _ => html! {},
    };

    let back_btn = if props.stage == WizardStage::MenubarHelp {
        html! {}
    } else {
        let nav_clone = nav;
        let handle_back = Callback::from(move |_| nav_clone.back());
        html! {
            <btn::Btn onclick={handle_back} classes={classes!("w-18")}>
                <icons::ChevronLeftIcon height="h-8" width="w-8" classes="ml-auto float-right"/>
                <div>{"Back"}</div>
            </btn::Btn>
        }
    };

    html! {
        <div class="py-4 px-8 bg-neutral-800 h-screen text-center flex flex-col gap-4">
            {content}
            <div class="mt-auto mb-2 flex flex-row gap-4">
                {back_btn}
                <btn::Btn onclick={handle_next} classes={classes!("w-full")}>
                    <div>{next_label.clone()}</div>
                    <icons::ChevronRightIcon height="h-8" width="w-8" classes="ml-auto float-right"/>
                </btn::Btn>
            </div>
        </div>
    }
}
