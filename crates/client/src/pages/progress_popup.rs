use serde_wasm_bindgen::from_value;
use shared::event::{ListenPayload, ModelStatusPayload};
use wasm_bindgen::{prelude::Closure, JsValue};
use yew::{platform::spawn_local, prelude::*};

pub struct ProgressPopup {
    msg: Option<String>,
    percent: Option<String>,
}

pub enum Msg {
    UpdateStatus(String, String),
}

impl Component for ProgressPopup {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();

        // Handle refreshing
        {
            let link = link.clone();
            spawn_local(async move {
                let cb = Closure::wrap(Box::new(move |payload: JsValue| {
                    if let Ok(res) = from_value::<ListenPayload<ModelStatusPayload>>(payload) {
                        link.send_message(Msg::UpdateStatus(res.payload.msg, res.payload.percent));
                    }
                }) as Box<dyn Fn(JsValue)>);

                let _ = crate::listen("progress_update", &cb).await;
                cb.forget();
            });
        }

        Self {
            msg: None,
            percent: None,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::UpdateStatus(msg, percent) => {
                self.msg = Some(msg);
                self.percent = Some(percent);
                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <div class="bg-neutral-800 text-white w-full h-screen">
                <div class="flex flex-col p-4">
                    { if let (Some(msg), Some(percent)) = (self.msg.clone(), self.percent.clone()) {
                        html! {
                            <>
                                <div class="text-xs pb-1">{format!("{} - {}%", msg.clone(), percent.clone())}</div>
                                <div class="w-full bg-stone-800 h-1 rounded-3xl text-xs">
                                    <div class="bg-cyan-400 h-1 rounded-lg pl-2 flex items-center animate-pulse" style={format!("width: {percent}%")}></div>
                                </div>
                            </>
                        }
                    } else { html! {
                        <div class="text-sm">{"Starting download..."}</div>
                    } }}
                </div>
            </div>
        }
    }
}
