use std::collections::HashMap;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::{
    components::{btn, icons, Header},
    invoke, save_user_settings,
    utils::RequestState,
};
use shared::event::ClientInvoke;
use shared::{FormType, SettingOpts};

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
    let (parent, _) = props
        .setting_ref
        .split_once('.')
        .unwrap_or((&props.setting_ref, ""));

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

    let label = if parent != "user" {
        html! {
            <>
                <span class="text-white">{format!("{}: ", parent)}</span>
                {props.opts.label.clone()}
            </>
        }
    } else {
        html! { props.opts.label.clone() }
    };

    html! {
        <div class="p-8">
            <div class="mb-2">
                <label class="text-yellow-500">{label}</label>
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
                {
                    match &props.opts.form_type {
                        FormType::List => {
                            html! {
                                <textarea
                                    onkeyup={onkeyup}
                                    class="form-input w-full text-sm rounded bg-stone-700 border-stone-800"
                                    rows="5"
                                >
                                    {(*value).clone()}
                                </textarea>
                            }
                        }
                        FormType::Text => {
                            html! {
                                <input
                                    onkeyup={onkeyup}
                                    type="text"
                                    class="form-input w-full text-sm rounded bg-stone-700 border-stone-800"
                                    value={(*value).clone()}
                                />
                            }
                        }
                    }
                }
            </div>
        </div>
    }
}

#[function_component(UserSettingsPage)]
pub fn user_settings_page() -> Html {
    let current_settings: UseStateHandle<Vec<(String, SettingOpts)>> = use_state_eq(Vec::new);
    let changes: UseStateHandle<HashMap<String, String>> = use_state_eq(HashMap::new);

    let req_state = use_state_eq(|| RequestState::NotStarted);
    if *req_state == RequestState::NotStarted {
        req_state.set(RequestState::InProgress);
        let current_settings = current_settings.clone();
        spawn_local(async move {
            if let Ok(res) = invoke(ClientInvoke::LoadUserSettings.as_ref(), JsValue::NULL).await {
                if let Ok(deser) = JsValue::into_serde::<Vec<(String, SettingOpts)>>(&res) {
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

    let handle_show_folder = Callback::from(|_| {
        spawn_local(async move {
            let _ = invoke(ClientInvoke::OpenSettingsFolder.as_ref(), JsValue::NULL).await;
        });
    });

    let handle_save_changes = {
        let has_changes = has_changes.clone();
        Callback::from(move |_| {
            let changes_ref = changes.clone();
            let updated = (*changes).clone();
            spawn_local(async move {
                // Send changes to backend to be validated & saved.
                if let Ok(ser) = JsValue::from_serde(&updated.clone()) {
                    // TODO: Handle any validation errors from backend and show
                    // them to user.
                    let _ = save_user_settings(ser).await;
                }
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
                <btn::Btn onclick={handle_show_folder}>
                    <icons::FolderOpenIcon classes={classes!("mr-2")}/>
                    {"Settings folder"}
                </btn::Btn>
                <btn::Btn onclick={handle_save_changes} disabled={!*has_changes}>
                    {"Save Changes"}
                </btn::Btn>
            </Header>
            <div>{contents}</div>
        </div>
    }
}
