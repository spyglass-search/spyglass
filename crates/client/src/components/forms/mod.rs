use web_sys::HtmlInputElement;
use yew::prelude::*;

use shared::form::{FormType, SettingOpts};

mod pathlist;
mod stringlist;

pub use pathlist::*;
pub use stringlist::*;

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
}

pub enum Msg {
    HandleInput
}

pub struct FormElement {
    node_ref: NodeRef,
    onchange: Callback<SettingChangeEvent>,
}

impl Component for FormElement {
    type Message = Msg;
    type Properties = FormProps;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        Self {
            onchange: props.onchange.clone(),
            node_ref: NodeRef::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::HandleInput => {
                let input: HtmlInputElement = self.node_ref.cast().expect("node ref not set");
                self.onchange.emit(SettingChangeEvent {
                    setting_name: props.setting_name.clone(),
                    new_value: input.value(),
                });
            }
        }
        false
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let props = ctx.props();

        let (parent, _) = props.setting_name
            .split_once('.')
            .unwrap_or((&props.setting_name, ""));

        // System settings have "_" as the parent.
        let label = html! {
            <>
                {
                    if parent != "_" {
                        html! { <span class="text-white">{format!("{}: ", parent)}</span> }
                    } else {
                        html! {}
                    }
                }
                {props.setting_name.clone()}
            </>
        };

        let onchange = props.onchange.clone();
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
                                    <PathList
                                        name={props.setting_name.clone()}
                                        value={props.opts.value.clone()}
                                        onchange={Callback::from(move |evt| onchange.emit(evt))}
                                    />
                                }
                            }
                            FormType::StringList => {
                                html! {
                                    <StringList
                                        name={props.setting_name.clone()}
                                        value={props.opts.value.clone()}
                                        onchange={Callback::from(move |evt| onchange.emit(evt))}
                                    />
                                }
                            }
                            FormType::Text | FormType::Path => {
                                html! {
                                    <input
                                        ref={self.node_ref.clone()}
                                        spellcheck="false"
                                        oninput={link.callback(|_| Msg::HandleInput)}
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
}