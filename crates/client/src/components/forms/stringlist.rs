use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::components::forms::SettingChangeEvent;
use crate::components::{btn, icons};

#[derive(Properties, PartialEq)]
pub struct StringListProps {
    pub name: String,
    pub value: String,
    pub onchange: Callback<SettingChangeEvent>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Add(String),
    HandleAdd,
    Remove(String),
}

pub struct StringList {
    pub values: Vec<String>,
    pub new_value_input: NodeRef,
}

impl StringList {
    pub fn emit_onchange(&self, ctx: &Context<Self>) {
        let props = ctx.props();

        if let Ok(new_value) = serde_json::to_string(&self.values) {
            props.onchange.emit(SettingChangeEvent {
                setting_name: props.name.clone(),
                new_value,
            });
        }
    }
}

impl Component for StringList {
    type Message = Msg;
    type Properties = StringListProps;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let mut values =
            serde_json::from_str::<Vec<String>>(&props.value).map_or(Vec::new(), |x| x);
        values.sort();

        Self {
            values,
            new_value_input: NodeRef::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();

        match msg {
            Msg::Add(new_value) => {
                self.values.push(new_value);
                self.values.sort();
                self.emit_onchange(ctx);

                true
            }
            Msg::HandleAdd => {
                if let Some(el) = self.new_value_input.cast::<HtmlInputElement>() {
                    link.send_message(Msg::Add(el.value()));
                    el.set_value("");
                }

                false
            }
            Msg::Remove(value) => {
                self.values.retain(|s| **s != value);
                self.emit_onchange(ctx);

                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let list_html = self.values.iter().map(|val| {
            let val = val.clone();
            let rm_msg = Msg::Remove(val.clone());

            html!  {
                <div class="flex items-center rounded-md p-1.5">
                    <div class={classes!("grow", "text-sm")}>{val.clone()}</div>
                    <button class={classes!("flex-none", "group")} onclick={link.callback(move |_| rm_msg.clone())}>
                        <icons::TrashIcon
                            height="h-5"
                            width="w-5"
                            classes={classes!("stroke-slate-400", "group-hover:stroke-white", "group-hover:fill-red-400")}
                        />
                    </button>
                </div>
            }
        })
        .collect::<Html>();

        html! {
            <div>
                <div class="border-1 rounded-md bg-stone-700 p-2 h-40 overflow-y-auto">
                    {list_html}
                </div>
                <div class="mt-4 flex flex-row gap-4">
                    <input
                        ref={self.new_value_input.clone()}
                        type="text"
                        class="form-input text-sm rounded bg-stone-700 border-stone-800"
                        placeholder="html"
                    />
                    <btn::Btn onclick={link.callback(|_| Msg::HandleAdd)}>
                        <icons::PlusIcon classes="mr-1" />
                        {"Add File Type"}
                    </btn::Btn>
                </div>
            </div>
        }
    }
}
