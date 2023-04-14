use std::str::FromStr;

use shared::keyboard::KeyCode;
use ui_components::btn::{Btn, BtnType};
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Clone, Debug)]
pub enum Msg {
    HandleKeyboardEvent(KeyboardEvent),
    HandleSearch,
}

pub struct SearchPage {
    search_wrapper_ref: NodeRef,
    search_input_ref: NodeRef,
    status_msg: Option<String>,
}

impl Component for SearchPage {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &yew::Context<Self>) -> Self {
        Self {
            search_input_ref: Default::default(),
            search_wrapper_ref: Default::default(),
            status_msg: None,
        }
    }

    fn update(&mut self, ctx: &yew::Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::HandleKeyboardEvent(event) => {
                let key = event.key();
                if let Ok(code) = KeyCode::from_str(&key.to_uppercase()) {
                    if code == KeyCode::Enter {
                        log::info!("key-code: {code}");
                        link.send_message(Msg::HandleSearch);
                    }
                }
            }
            Msg::HandleSearch => {
                let query = self
                    .search_input_ref
                    .cast::<HtmlInputElement>()
                    .map(|x| x.value());

                log::info!("handling search! {:?}", query);
                if let Some(query) = query {
                    self.status_msg = Some(format!("searching: {query}"));
                }
            }
        }
        false
    }

    fn view(&self, ctx: &yew::Context<Self>) -> yew::Html {
        let link = ctx.link();

        html! {
            <div ref={self.search_wrapper_ref.clone()} class="relative">
                <div class="flex flex-nowrap w-full bg-neutral-800 p-4">
                    <input
                        ref={self.search_input_ref.clone()}
                        id="searchbox"
                        type="text"
                        class="bg-neutral-800 text-white text-2xl py-3 overflow-hidden flex-1 outline-none active:outline-none focus:outline-none caret-white placeholder-neutral-600"
                        placeholder="how do i resize a window in tauri?"
                        spellcheck="false"
                        tabindex="-1"
                        onkeyup={link.callback(Msg::HandleKeyboardEvent)}
                    />
                    <Btn
                        _type={BtnType::Primary}
                        onclick={link.callback(|_| Msg::HandleSearch)}
                    >
                        {"Search"}
                    </Btn>
                </div>
                <div class="border-t-2 border-neutral-900 p-4">
                    {self.status_msg.clone().unwrap_or_else(|| "how to guide?".into())}
               </div>
            </div>
        }
    }
}
