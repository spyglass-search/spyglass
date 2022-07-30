use yew::prelude::*;

use crate::components::{btn, Header};

#[derive(PartialEq)]
pub enum FormType {
    Text,
}

#[derive(Properties, PartialEq)]
pub struct SettingFormProps {
    form_type: FormType,
    help_text: Option<String>,
    label: String,
    value: String,
}

#[function_component(SettingForm)]
pub fn setting_form(props: &SettingFormProps) -> Html {
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
                    type="text"
                    class="w-full text-sm rounded bg-stone-700 border-stone-800"
                    value={props.value.clone()}
                />
            </div>
        </div>
    }
}

#[function_component(UserSettingsPage)]
pub fn user_settings_page() -> Html {
    let callback = Callback::from(|_| {
        log::info!("{}", "adklfjakdl");
    });

    html! {
        <div class="text-white">
            <Header label="User Settings">
                <btn::Btn onclick={callback}>
                    {"Save Changes"}
                </btn::Btn>
            </Header>
            <div>
                <SettingForm
                    label="Data directory"
                    value={"/Users/a5huynh/Library/Application Support/com.athlabs.spyglass-dev"}
                    form_type={FormType::Text}
                    help_text={"The data directory is where your index, lenses, plugins, and logs are stored."}
                />
            </div>
        </div>
    }
}
