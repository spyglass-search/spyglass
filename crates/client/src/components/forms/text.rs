use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::components::forms::SettingChangeEvent;

pub enum Msg {
    HandleInput,
}

#[derive(Properties, PartialEq)]
pub struct TextProps {
    pub name: String,
    pub value: String,
    pub onchange: Callback<SettingChangeEvent>,
}

pub struct Text {
    value: String,
    node_ref: NodeRef,
}

impl Component for Text {
    type Message = Msg;
    type Properties = TextProps;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        Self {
            value: props.value.clone(),
            node_ref: NodeRef::default(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::HandleInput => {
                if let Some(el) = self.node_ref.cast::<HtmlInputElement>() {
                    self.value = el.value();
                }

                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        html! {
            <input
                ref={self.node_ref.clone()}
                spellcheck="false"
                oninput={link.callback(|_| Msg::HandleInput)}
                value={self.value.clone()}
                type="text"
                class="form-input w-full text-sm rounded bg-stone-700 border-stone-800"
            />
        }
    }
}
