use shared::{
    accelerator,
    keyboard::{KeyCode, ModifiersState},
};
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::components::{KeyComponent, ModifierIcon};

use super::FormFieldProps;
use crate::components::forms::SettingChangeEvent;
use std::str::FromStr;

pub enum Msg {
    HandleInput,
    KeyDown(KeyboardEvent),
}

pub struct KeyBinding {
    value: String,
    node_ref: NodeRef,
}

impl Component for KeyBinding {
    type Message = Msg;
    type Properties = FormFieldProps;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        Self {
            value: props.value.clone(),
            node_ref: NodeRef::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();

        match msg {
            Msg::HandleInput => {
                if let Some(el) = self.node_ref.cast::<HtmlInputElement>() {
                    self.value = el.value();
                    props.onchange.emit(SettingChangeEvent {
                        setting_name: props.name.clone(),
                        new_value: self.value.clone(),
                        restart_required: props.restart_required,
                    });
                }

                true
            }
            Msg::KeyDown(evt) => {
                if evt.alt_key() || evt.ctrl_key() || evt.meta_key() {
                    let mut val = String::from("");
                    if evt.meta_key() {
                        val.push_str("Cmd+");
                    }

                    if evt.ctrl_key() {
                        val.push_str("Ctrl+");
                    }

                    if evt.alt_key() {
                        val.push_str("Alt+");
                    }

                    if evt.shift_key() {
                        val.push_str("Shift+");
                    }

                    let mut modifier = ModifiersState::empty();
                    modifier.set(ModifiersState::ALT, evt.alt_key());
                    modifier.set(ModifiersState::CONTROL, evt.ctrl_key());
                    modifier.set(ModifiersState::SHIFT, evt.shift_key());
                    modifier.set(ModifiersState::SUPER, evt.meta_key());

                    if let Ok(key) = KeyCode::from_str(evt.key().to_uppercase().as_str()) {
                        match key {
                            KeyCode::Unidentified(_) => {
                                if let Ok(key) =
                                    KeyCode::from_str(evt.code().to_uppercase().as_str())
                                {
                                    match key {
                                        KeyCode::Unidentified(_) => (),
                                        _ => {
                                            val.push_str(key.to_str());
                                            evt.prevent_default();
                                        }
                                    }
                                }
                            }
                            _ => {
                                val.push_str(key.to_str());
                            }
                        }
                    }

                    self.value = val;

                    props.onchange.emit(SettingChangeEvent {
                        setting_name: props.name.clone(),
                        new_value: self.value.clone(),
                        restart_required: props.restart_required,
                    });
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let key_binding = if let Ok(acc) = accelerator::parse_accelerator(
            self.value.as_str(),
            crate::utils::get_os().to_string().as_str(),
        ) {
            html! {
                <>
                  <ModifierIcon modifier={acc.mods}></ModifierIcon>
                  <KeyComponent>{acc.key.to_str()}</KeyComponent>
                </>
            }
        } else {
            html! {
                <KeyComponent>{"Invalid"}</KeyComponent>
            }
        };

        html! {
            <div class="w-full flex flex-row items-center gap-1">
                <input
                    ref={self.node_ref.clone()}
                    spellcheck="false"
                    oninput={link.callback(|_| Msg::HandleInput)}
                    onkeydown={link.callback(Msg::KeyDown)}
                    value={self.value.clone()}
                    type="text"
                    class="grow form-input w-full text-sm rounded bg-stone-700 border-stone-800"
                />

                {key_binding}
            </div>
        }
    }
}
