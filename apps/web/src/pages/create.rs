use ui_components::btn::{Btn, BtnType};
use yew::prelude::*;

pub struct CreateLensPage {
    sources: Vec<String>,
}

#[derive(Properties, PartialEq)]
pub struct CreateLensProps {}

pub enum Msg {}


impl Component for CreateLensPage {
    type Message = Msg;
    type Properties = CreateLensProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            sources: Vec::new()
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div>
                <div class="px-6 pt-4 flex flex-row items-center">
                    <h2 class="bold text-xl ">{"New Lens"}</h2>
                    <Btn classes="ml-auto" _type={BtnType::Success}>{"Save"}</Btn>
                </div>
                <div class="flex flex-col gap-8 px-6 py-4">
                    <div class="flex flex-row gap-4">
                        <Btn>{"Add data from URL"}</Btn>
                        <Btn>{"Add data from Google Drive"}</Btn>
                    </div>
                    <div class="flex flex-col">
                        <div class="mb-2 text-sm font-semibold uppercase text-cyan-500">{"Sources"}</div>
                    </div>
                </div>
            </div>
        }
    }
}