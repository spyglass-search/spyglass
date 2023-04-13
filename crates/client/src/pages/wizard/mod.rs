use strum_macros::{Display, EnumString};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::hooks::use_navigator;

use crate::components::{btn, forms::SettingChangeEvent};
use crate::{tauri_invoke, Route};
use shared::event::{ClientInvoke, WizardFinishedParams};
use ui_components::icons;

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
    #[strum(serialize = "index-bookmarks")]
    IndexBookmarks,
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
    let toggle_audio_transcription = use_state(|| false);

    let cur_stage = props.stage.clone();
    let nav_clone = nav.clone();

    let tfi_state = toggle_file_indexer.clone();
    let tat_state = toggle_audio_transcription.clone();
    let handle_next = Callback::from(move |_| {
        let next_stage = match cur_stage {
            WizardStage::MenubarHelp => WizardStage::DisplaySearchbarHelp,
            WizardStage::DisplaySearchbarHelp => WizardStage::IndexFiles,
            WizardStage::IndexFiles => WizardStage::IndexCloud,
            WizardStage::IndexCloud => WizardStage::IndexBookmarks,
            WizardStage::IndexBookmarks => WizardStage::IndexWeb,
            WizardStage::IndexWeb => WizardStage::Done,
            _ => WizardStage::Done,
        };

        if next_stage == WizardStage::Done {
            let params = WizardFinishedParams {
                toggle_audio_transcription: *tat_state,
                toggle_file_indexer: *tfi_state,
            };

            spawn_local(async move {
                let _ = tauri_invoke::<_, ()>(ClientInvoke::WizardFinished, &params).await;
            });
            return;
        }

        nav_clone.push(&Route::Wizard { stage: next_stage });
    });

    let tfi_state = toggle_file_indexer.clone();
    let tat_state = toggle_audio_transcription.clone();
    let handle_onchange = Callback::from(move |event: SettingChangeEvent| {
        if event.setting_name == "_.file-indexer" {
            if let Ok(new_value) = serde_json::from_str::<bool>(&event.new_value) {
                tfi_state.set(new_value);
            }
        } else if event.setting_name == "_.audio-transcription" {
            if let Ok(new_value) = serde_json::from_str::<bool>(&event.new_value) {
                tat_state.set(new_value);
            }
        }
    });

    let content = match props.stage {
        WizardStage::MenubarHelp => {
            html! { <menubar_help::MenubarHelpPage /> }
        }
        WizardStage::DisplaySearchbarHelp => {
            html! { <display_searchbar::DisplaySearchbarPage /> }
        }
        WizardStage::IndexBookmarks => {
            html! { <indexing_help::IndexBookmarks /> }
        }
        WizardStage::IndexFiles => {
            html! {
                <indexing_help::IndexFilesHelp
                    toggle_file_indexer={*toggle_file_indexer}
                    toggle_audio_transcription={*toggle_audio_transcription}
                    onchange={handle_onchange}
                />
            }
        }
        WizardStage::IndexCloud => {
            html! { <indexing_help::IndexCloudHelp /> }
        }
        WizardStage::IndexWeb => {
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
                    <div>{"Next"}</div>
                    <icons::ChevronRightIcon height="h-8" width="w-8" classes="ml-auto float-right"/>
                </btn::Btn>
            </div>
        </div>
    }
}
