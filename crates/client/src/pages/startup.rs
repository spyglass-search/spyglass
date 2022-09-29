use gloo::timers::callback::Interval;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use shared::event::ClientInvoke;

use crate::components::icons;
use crate::invoke;

pub struct StartupPage {
    progress_caption: String,
    time_taken: i32,
    handle: Option<Interval>,
}

pub enum Msg {
    Done,
    CheckStatus,
    UpdateStatus(String),
}

impl Component for StartupPage {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let interval_handle = {
            let link = ctx.link().clone();
            Interval::new(1_000, move || link.send_message(Msg::CheckStatus))
        };

        Self {
            handle: Some(interval_handle),
            progress_caption: "".to_string(),
            time_taken: 0,
        }
    }

    fn destroy(&mut self, _: &Context<Self>) {
        if let Some(interval) = self.handle.take() {
            interval.cancel();
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::UpdateStatus(value) => {
                self.progress_caption = value;
            }
            Msg::CheckStatus => {
                self.time_taken += 1;
                let link = ctx.link().clone();
                spawn_local(async move {
                    if let Ok(value) =
                        invoke(ClientInvoke::GetStartupProgressText.as_ref(), JsValue::NULL).await
                    {
                        if let Some(prog) = value.as_string() {
                            if prog == "DONE" {
                                link.send_message(Msg::Done);
                            } else {
                                link.send_message(Msg::UpdateStatus(prog))
                            }
                        }
                    }
                });
            }
            Msg::Done => {
                if let Some(interval) = self.handle.take() {
                    interval.cancel();
                }
            }
        }

        true
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let time_taken = if self.time_taken > 0 {
            format!("{}s", self.time_taken)
        } else {
            "".to_string()
        };

        html! {
            <div class="bg-neutral-800 flex flex-col place-content-center place-items-center mt-14">
                <icons::RefreshIcon animate_spin={true} height="h-16" width="w-16" />
                <div class="mt-4 font-medium">{"Starting Spyglass"}</div>
                <div class="mt-1 text-stone-500 text-sm">{self.progress_caption.clone()}</div>
                <div class="mt-1 text-stone-500 text-sm">{time_taken}</div>
            </div>
        }
    }
}
