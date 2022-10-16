use yew::prelude::*;

use shared::form::{FormType, SettingOpts};

mod pathlist;
mod stringlist;
mod text;
mod toggle;

pub use pathlist::*;
pub use stringlist::*;
pub use text::*;
pub use toggle::*;

#[derive(Clone)]
pub struct SettingChangeEvent {
    pub setting_name: String,
    pub new_value: String,
}

#[derive(Properties, PartialEq)]
pub struct FormProps {
    #[prop_or_default]
    pub onchange: Callback<SettingChangeEvent>,
    pub setting_name: String,
    pub opts: SettingOpts,
    #[prop_or_default]
    pub error_msg: Option<String>,
}

pub struct FormElement {
    onchange: Callback<SettingChangeEvent>,
    opts: SettingOpts,
}

impl FormElement {
    fn alignment(&self) -> String {
        match self.opts.form_type {
            FormType::Bool => "flex-row".to_string(),
            _ => "flex-col".to_string(),
        }
    }

    fn element(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let onchange = self.onchange.clone();

        match &self.opts.form_type {
            FormType::Bool => {
                html! {
                    <Toggle
                        name={props.setting_name.clone()}
                        value={self.opts.value.clone()}
                        onchange={Callback::from(move |evt| onchange.emit(evt))}
                    />
                }
            }
            FormType::Number => {
                html! {
                    <Text
                        name={props.setting_name.clone()}
                        value={self.opts.value.clone()}
                        onchange={Callback::from(move |evt| onchange.emit(evt))}
                    />
                }
            }
            FormType::PathList => {
                html! {
                    <PathList
                        name={props.setting_name.clone()}
                        value={self.opts.value.clone()}
                        onchange={Callback::from(move |evt| onchange.emit(evt))}
                    />
                }
            }
            FormType::StringList => {
                html! {
                    <StringList
                        name={props.setting_name.clone()}
                        value={self.opts.value.clone()}
                        onchange={Callback::from(move |evt| onchange.emit(evt))}
                    />
                }
            }
            FormType::Text | FormType::Path => {
                html! {
                    <Text
                        name={props.setting_name.clone()}
                        value={self.opts.value.clone()}
                        onchange={Callback::from(move |evt| onchange.emit(evt))}
                    />
                }
            }
        }
    }
}

impl Component for FormElement {
    type Message = ();
    type Properties = FormProps;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        Self {
            onchange: props.onchange.clone(),
            opts: props.opts.clone(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, _msg: Self::Message) -> bool {
        false
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let (parent, _) = props
            .setting_name
            .split_once('.')
            .unwrap_or((&props.setting_name, ""));

        // Show a label w/ the "parent name: label", i.e.  "local-file-indexer: Folder List".
        // System settings have "_" as the parent and will show up as just the label.
        let label = if parent != "_" {
            html! {
                <>
                    <span class="text-white">{format!("{}: ", parent)}</span>
                    <span>{props.opts.label.clone()}</span>
                </>
            }
        } else {
            html! { <span>{props.opts.label.clone()}</span> }
        };

        html! {
            <div class={classes!("px-8", "mb-8", "flex", self.alignment())}>
                <div class="mb-2">
                    <label class="text-yellow-500">{label}</label>
                    {
                        if let Some(help_text) = props.opts.help_text.clone() {
                            html! {
                                <div class="text-gray-500 text-sm">
                                    {help_text.clone()}
                                </div>
                            }
                        } else {
                            html! { }
                        }
                    }
                    {if let Some(msg) = props.error_msg.clone() {
                        html! {
                            <div class="text-red-500 text-xs py-2">{msg}</div>
                        }
                    } else {
                        html! {}
                    }}

                </div>
                {self.element(ctx)}
            </div>
        }
    }
}
