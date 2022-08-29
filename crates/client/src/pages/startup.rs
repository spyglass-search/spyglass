use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use shared::event::ClientEvent;

use crate::components::icons;
use crate::listen;

#[derive(Debug, Deserialize)]
struct ListenPayload {
    payload: String,
}

#[derive(Properties, PartialEq, Eq)]
pub struct StartupPageProps {
    #[prop_or_default]
    pub status_caption: String,
}

#[function_component(StartupPage)]
pub fn startup_page(props: &StartupPageProps) -> Html {
    let status_caption = use_state_eq(|| props.status_caption.clone());
    {
        // Refresh search results
        let status_caption = status_caption.clone();
        spawn_local(async move {
            let cb = Closure::wrap(Box::new(move |payload: JsValue| {
                if let Ok(obj) = payload.into_serde::<ListenPayload>() {
                    status_caption.set(obj.payload);
                }
            }) as Box<dyn Fn(JsValue)>);

            let _ = listen(ClientEvent::StartupProgress.as_ref(), &cb).await;
            cb.forget();
        });
    }

    html! {
        <div class="flex flex-col place-content-center place-items-center mt-14">
            <icons::RefreshIcon animate_spin={true} height="h-16" width="w-16" />
            <div class="mt-4 font-medium">{"Starting Spyglass"}</div>
            <div class="mt-1 text-stone-500 text-sm">{(*status_caption).clone()}</div>
        </div>
    }
}
