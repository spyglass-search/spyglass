use yew::prelude::*;


#[derive(Clone, Debug)]
pub enum Msg {
    Start
}

pub struct SearchPage;

impl Component for SearchPage {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &yew::Context<Self>) -> Self {
        Self
    }

    fn update(&mut self, _ctx: &yew::Context<Self>, _msg: Self::Message) -> bool {
        false
    }

    fn view(&self, _ctx: &yew::Context<Self>) -> yew::Html {
        html! { }
    }
}