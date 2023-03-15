use yew::prelude::*;

use super::FormFieldProps;
use crate::components::forms::SettingChangeEvent;
pub enum Msg {
    Toggle,
}
pub struct Toggle {
    state: bool,
}

impl Component for Toggle {
    type Message = Msg;
    type Properties = FormFieldProps;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let state = serde_json::from_str::<bool>(&props.value).map_or(false, |x| x);

        Self { state }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();

        match msg {
            Msg::Toggle => {
                self.state = !self.state;
                if let Ok(new_value) = serde_json::to_string(&self.state) {
                    props.onchange.emit(SettingChangeEvent {
                        setting_name: props.name.clone(),
                        new_value,
                        restart_required: props.restart_required,
                    });
                }

                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let link = ctx.link();
        let toggle_id = format!("toggle_{}", props.name);

        let label = if self.state { "Y" } else { "N" };

        html! {
            <div class="grow items-center pl-4 justify-end flex">
                <label for={toggle_id.clone()} class="items-center cursor-pointer">
                    <div class="relative">
                        <input type="checkbox" id={toggle_id} class="sr-only" checked={self.state} onchange={link.callback(|_| Msg::Toggle)} />
                        <div class="block bg-stone-700 w-14 h-8 rounded-full"></div>
                        <div class="text-black dot absolute left-1 top-1 bg-white w-6 h-6 rounded-full transition text-center">
                            {label}
                        </div>
                    </div>
                </label>
            </div>
        }
    }
}
