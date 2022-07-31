use std::collections::HashMap;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::{components::{btn, Header}, save_settings};

#[derive(PartialEq)]
pub enum FormType {
    Text,
}

#[derive(Properties, PartialEq)]
pub struct SettingFormProps {
    #[prop_or_default]
    onchange: Callback<KeyboardEvent>,
    form_type: FormType,
    help_text: Option<String>,
    label: String,
    value: String,
}

#[function_component(SettingForm)]
pub fn setting_form(props: &SettingFormProps) -> Html {
    let onkeyup = {
        let cur_value = props.value.clone();
        let onchange = props.onchange.clone();
        Callback::from(move |e: KeyboardEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let value = input.value();
            if value != cur_value {
                onchange.emit(e);
            }
        })
    };
    html! {
        <div class="p-8">
            <div class="mb-2">
                <label class="text-yellow-500">{props.label.clone()}</label>
                {
                    if let Some(help_text) = props.help_text.clone() {
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
                    value={props.value.clone()}
                />
            </div>
        </div>
    }
}

#[function_component(UserSettingsPage)]
pub fn user_settings_page() -> Html {
    // Detect changes in setting values & enable the save changes button
    let has_changes = use_state_eq(|| false);
    let onchange = {
        let has_changes = has_changes.clone();
        Callback::from(move |_| {
            has_changes.set(true);
        })
    };

    let handle_save_changes = {
        let has_changes = has_changes.clone();
        Callback::from(move |_| {
            let mut example: HashMap<String, String> = HashMap::new();
            example.insert("test".into(), "/user/a5huynh/documents".into());
            spawn_local(async move {
                let _ = save_settings(JsValue::from_serde(&example).expect("cant serialize")).await;
            });

            has_changes.set(false);
        })
    };

    html! {
        <div class="text-white">
            <Header label="User Settings">
                <btn::Btn onclick={handle_save_changes} disabled={!*has_changes}>
                    {"Save Changes"}
                </btn::Btn>
            </Header>
            <div>
                <SettingForm
                    onchange={onchange}
                    label="Data directory"
                    value={"/Users/a5huynh/Library/Application Support/com.athlabs.spyglass-dev"}
                    form_type={FormType::Text}
                    help_text={"The data directory is where your index, lenses, plugins, and logs are stored."}
                />
            </div>
        </div>
    }
}
