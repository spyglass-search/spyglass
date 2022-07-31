use std::collections::HashMap;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::{
    components::{btn, Header},
    invoke, save_user_settings,
    utils::RequestState,
};
use shared::event::ClientInvoke;
use shared::SettingOpts;

#[derive(Properties, PartialEq)]
pub struct SettingFormProps {
    #[prop_or_default]
    onchange: Callback<SettingChangeEvent>,
    setting_ref: String,
    opts: SettingOpts,
}

pub struct SettingChangeEvent {
    setting_ref: String,
    new_value: String,
}

#[function_component(SettingForm)]
pub fn setting_form(props: &SettingFormProps) -> Html {
    let value = use_state(|| props.opts.value.clone());

    let onkeyup = {
        let onchange = props.onchange.clone();
        let setting_ref = props.setting_ref.clone();
        let value = value.clone();
        Callback::from(move |e: KeyboardEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let input_value = input.value();
            onchange.emit(SettingChangeEvent {
                setting_ref: setting_ref.clone(),
                new_value: input_value.clone(),
            });
            value.set(input_value);
        })
    };

    html! {
        <div class="p-8">
            <div class="mb-2">
                <label class="text-yellow-500">{props.opts.label.clone()}</label>
                {
                    if let Some(help_text) = props.opts.help_text.clone() {
                        html! {
                            <div>
                                <small class="text-gray-500">
                                    {help_text.clone()}
                                </small>
                            </div>
                        }
                    } else {
                        html! { }
                    }
                }
            </div>
            <div>
                <input
                    onkeyup={onkeyup}
                    type="text"
                    class="form-input w-full text-sm rounded bg-stone-700 border-stone-800"
                    value={(*value).clone()}
                />
            </div>
        </div>
    }
}

#[function_component(UserSettingsPage)]
pub fn user_settings_page() -> Html {
    let current_settings: UseStateHandle<HashMap<String, SettingOpts>> = use_state_eq(HashMap::new);
    let changes: UseStateHandle<HashMap<String, String>> = use_state_eq(HashMap::new);

    let req_state = use_state_eq(|| RequestState::NotStarted);
    if *req_state == RequestState::NotStarted {
        req_state.set(RequestState::InProgress);
        let current_settings = current_settings.clone();
        spawn_local(async move {
            if let Ok(res) = invoke(ClientInvoke::LoadUserSettings.as_ref(), JsValue::NULL).await {
                if let Ok(deser) = JsValue::into_serde::<HashMap<String, SettingOpts>>(&res) {
                    current_settings.set(deser);
                } else {
                    log::error!("unable to deserialize");
                }
            } else {
                log::error!("unable to invoke");
            }
        })
    }

    // Detect changes in setting values & enable the save changes button
    let has_changes = use_state_eq(|| false);
    let onchange = {
        let has_changes = has_changes.clone();
        let changes = changes.clone();
        Callback::from(move |evt: SettingChangeEvent| {
            has_changes.set(true);
            let mut updated = (*changes).clone();
            updated.insert(evt.setting_ref, evt.new_value);
            changes.set(updated);
        })
    };

    let handle_save_changes = {
        let has_changes = has_changes.clone();
        Callback::from(move |_| {
            let changes_ref = changes.clone();
            let updated = (*changes).clone();
            spawn_local(async move {
                let _ = save_user_settings(
                    JsValue::from_serde(&updated.clone()).expect("cant serialize"),
                )
                .await;
            });

            changes_ref.set(HashMap::new());
            has_changes.set(false);
        })
    };

    let contents = current_settings
        .iter()
        .map(|(setting_ref, setting)| {
            html! {
                <SettingForm onchange={onchange.clone()} setting_ref={setting_ref.clone()} opts={setting.clone()} />
            }
        })
        .collect::<Html>();

    html! {
        <div class="text-white">
            <Header label="User Settings">
                <btn::Btn onclick={handle_save_changes} disabled={!*has_changes}>
                    {"Save Changes"}
                </btn::Btn>
            </Header>
            <div>{contents}</div>
        </div>
    }
}
