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
    HandleInput,
}

pub struct FormElement {
    node_ref: NodeRef,
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
        let link = ctx.link();
        let props = ctx.props();
        let onchange = self.onchange.clone();

        match &self.opts.form_type {
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
                    <input
                        ref={self.node_ref.clone()}
                        spellcheck="false"
                        oninput={link.callback(|_| Msg::HandleInput)}
                        type="text"
                        class="form-input w-full text-sm rounded bg-stone-700 border-stone-800"
                    />
                }
            }
            FormType::Bool => {
                html! {
                    <div class="grow items-center mt-2 justify-end flex">
                        <label for="toggle" class="items-center cursor-pointer">
                            <div class="relative">
                                <input type="checkbox" id="toggle" class="sr-only" />
                                <div class="block bg-stone-700 w-14 h-8 rounded-full"></div>
                                <div class="dot absolute left-1 top-1 bg-white w-6 h-6 rounded-full transition text-center">
                                    {"Y"}
                                </div>
                            </div>
                        </label>
                    </div>
                }
            }
        }
    }
}

impl Component for FormElement {
    type Message = Msg;
    type Properties = FormProps;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        Self {
            onchange: props.onchange.clone(),
            opts: props.opts.clone(),
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
                </div>
                {self.element(ctx)}
            </div>
        }
    }
}
