use std::collections::HashMap;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::{
    components::{btn, forms, icons, Header},
    invoke, save_user_settings,
    utils::RequestState,
};
use shared::event::ClientInvoke;
use shared::form::{FormType, SettingOpts};

#[derive(Properties, PartialEq)]
pub struct SettingFormProps {
    #[prop_or_default]
    onchange: Callback<SettingChangeEvent>,
    setting_ref: String,
    opts: SettingOpts,
}

#[derive(Clone)]
pub struct SettingChangeEvent {
    setting_ref: String,
    new_value: String,
}

#[function_component(SettingForm)]
pub fn setting_form(props: &SettingFormProps) -> Html {
    let input_ref = use_node_ref();

    {
        let input_ref = input_ref.clone();
        let value = props.opts.value.clone();
        use_effect(move || {
            if let Some(el) = input_ref.cast::<HtmlInputElement>() {
                // Only set the input once on render
                if el.value().is_empty() && !value.is_empty() {
                    el.set_value(&value);
                }
            }

            || {}
        });
    }

    let (parent, _) = props
        .setting_ref
        .split_once('.')
        .unwrap_or((&props.setting_ref, ""));

    let oninput = {
        let onchange = props.onchange.clone();
        let setting_ref = props.setting_ref.clone();
        Callback::from(move |e: InputEvent| {
            e.stop_immediate_propagation();
            let input: HtmlInputElement = e.target_unchecked_into();
            let input_value = input.value();
            onchange.emit(SettingChangeEvent {
                setting_ref: setting_ref.clone(),
                new_value: input_value,
            });
        })
    };

    // System settings have "_" as the parent.
    let label = if parent != "_" {
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
        <div class="px-8 mb-8">
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
                        FormType::PathList => {
                            html! {
                                <forms::PathList value={props.opts.value.clone()} oninput={oninput} />
                            }
                        }
                        FormType::StringList => {
                            html! {
                                <textarea
                                    ref={input_ref.clone()}
                                    spellcheck="false"
                                    oninput={oninput}
                                    class="form-input w-full text-sm rounded bg-stone-700 border-stone-800"
                                    rows="5"
                                    placeholder={"[\"/Users/example/Documents\", \"/Users/example/Desktop/Notes\"]"}
                                >
                                </textarea>
                            }
                        }
                        FormType::Text | FormType::Path => {
                            html! {
                                <input
                                    ref={input_ref.clone()}
                                    spellcheck="false"
                                    oninput={oninput}
                                    type="text"
                                    class="form-input w-full text-sm rounded bg-stone-700 border-stone-800"
                                />
                            }
                        }
                    }
                }
            </div>
        </div>
    }
}

#[derive(Clone)]
pub enum Msg {
    FetchSettings,
    HandleOnChange(SettingChangeEvent),
    HandleSave,
    HandleShowFolder,
    SetCurrentSettings(Vec<(String, SettingOpts)>),
}

pub struct UserSettingsPage {
    current_settings: Vec<(String, SettingOpts)>,
    changes: HashMap<String, String>,
    has_changes: bool,
    req_settings: RequestState,
}

impl UserSettingsPage {
    async fn fetch_user_settings() -> Vec<(String, SettingOpts)> {
        match invoke(ClientInvoke::LoadUserSettings.as_ref(), JsValue::NULL).await {
            Ok(results) => match serde_wasm_bindgen::from_value(results) {
                Ok(parsed) => parsed,
                Err(e) => {
                    log::error!("Unable to deserialize results: {}", e.to_string());
                    Vec::new()
                }
            }
            Err(e) => {
                log::error!("Error fetching user settings: {:?}", e);
                Vec::new()
            }
        }
    }
}

impl Component for UserSettingsPage {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();
        link.send_message(Msg::FetchSettings);

        Self {
            current_settings: Vec::new(),
            changes: HashMap::new(),
            has_changes: false,
            req_settings: RequestState::NotStarted
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::FetchSettings => {
                self.req_settings = RequestState::InProgress;
                link.send_future(async { Msg::SetCurrentSettings(UserSettingsPage::fetch_user_settings().await) });

                false
            }
            Msg::HandleOnChange(evt) => {
                self.has_changes = true;
                self.changes.insert(evt.setting_ref, evt.new_value);
                true
            }
            Msg::HandleSave => {
                let changes = self.changes.clone();
                spawn_local(async move {
                    // Send changes to backend to be validated & saved.
                    if let Ok(ser) = serde_wasm_bindgen::to_value(&changes) {
                        // TODO: Handle any validation errors from backend and show
                        // them to user.
                        let _ = save_user_settings(ser).await;
                    }
                });

                self.changes.clear();
                self.has_changes = false;
                true
            }
            Msg::HandleShowFolder => {
                spawn_local(async move {
                    let _ = invoke(ClientInvoke::OpenSettingsFolder.as_ref(), JsValue::NULL).await;
                });

                false
            }
            Msg::SetCurrentSettings(settings) => {
                self.current_settings = settings;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let contents = self.current_settings
            .iter()
            .map(|(setting_ref, setting)| {
                html! {
                    <SettingForm
                        onchange={link.callback(Msg::HandleOnChange)}
                        setting_ref={setting_ref.clone()}
                        opts={setting.clone()}
                    />
                }
            })
            .collect::<Html>();

        html! {
            <div class="text-white bg-neutral-800 h-full">
                <Header label="User Settings">
                    <btn::Btn onclick={link.callback(|_| Msg::HandleShowFolder)}>
                        <icons::FolderOpenIcon classes={classes!("mr-2")}/>
                        {"Settings folder"}
                    </btn::Btn>
                    <btn::Btn onclick={link.callback(|_| Msg::HandleSave)} disabled={!self.has_changes}>
                        {"Save & Restart"}
                    </btn::Btn>
                </Header>
                <div class="pt-8 bg-netural-800">
                    {contents}
                </div>
            </div>
        }
    }
}