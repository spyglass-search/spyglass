use std::collections::HashMap;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::forms::{FormElement, SettingChangeEvent};
use crate::{
    components::{btn, icons, Header},
    invoke, save_user_settings,
    utils::RequestState,
};
use shared::event::ClientInvoke;
use shared::form::SettingOpts;

#[derive(Clone)]
pub enum Msg {
    FetchSettings,
    HandleOnChange(SettingChangeEvent),
    HandleSave,
    HandleShowFolder,
    SetCurrentSettings(Vec<(String, SettingOpts)>),
    SetErrors(HashMap<String, String>),
}

pub struct UserSettingsPage {
    current_settings: Vec<(String, SettingOpts)>,
    errors: HashMap<String, String>,
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
            },
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
            errors: HashMap::new(),
            has_changes: false,
            req_settings: RequestState::NotStarted,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::FetchSettings => {
                self.req_settings = RequestState::InProgress;
                link.send_future(async {
                    Msg::SetCurrentSettings(UserSettingsPage::fetch_user_settings().await)
                });

                false
            }
            Msg::HandleOnChange(evt) => {
                self.has_changes = true;
                self.changes.insert(evt.setting_name, evt.new_value);
                true
            }
            Msg::HandleSave => {
                let changes = self.changes.clone();
                // Send changes to backend to be validated & saved.
                if let Ok(ser) = serde_wasm_bindgen::to_value(&changes) {
                    link.send_future(async move {
                        if let Err(res) = save_user_settings(ser).await {
                            if let Ok(errors) =
                                serde_wasm_bindgen::from_value::<HashMap<String, String>>(res)
                            {
                                log::debug!("save_user_settings: {:?}", errors);
                                return Msg::SetErrors(errors);
                            }
                        }

                        Msg::SetErrors(HashMap::new())
                    });
                }

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
            Msg::SetErrors(errors) => {
                self.errors = errors;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let contents = self
            .current_settings
            .iter()
            .map(|(setting_ref, setting)| {
                let error_msg = self.errors.get(setting_ref).map(|msg| msg.to_owned());

                html! {
                    <FormElement
                        error_msg={error_msg}
                        onchange={link.callback(Msg::HandleOnChange)}
                        opts={setting.clone()}
                        setting_name={setting_ref.clone()}
                    />
                }
            })
            .collect::<Html>();

        html! {
            <div>
                <Header label="User Settings">
                    <btn::Btn onclick={link.callback(|_| Msg::HandleShowFolder)}>
                        <icons::FolderOpenIcon classes={classes!("mr-2")}/>
                        {"Show Folder"}
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
