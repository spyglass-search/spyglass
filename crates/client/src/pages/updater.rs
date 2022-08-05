use js_sys::Math;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::{btn::Btn, icons, Header};
use crate::invoke;
use shared::event::ClientInvoke;

// Random gif for your viewing pleasure.
const UPDATE_GIFS: [&str; 6] = [
    // Adventure Time
    "10bxTLrpJNS0PC",
    "fm4WhPMzu9hRK",
    "13p77tfexyLtx6",
    "13FBIII8M4IDDi",
    // Futurama
    "gYZ7qO81g4dt6",
    // Elmo
    "MdqE46HziuFJTlIwjw",
];

#[function_component(UpdaterPage)]
pub fn updater_page() -> Html {
    let is_updating = use_state_eq(|| false);

    let is_updating_ref = is_updating.clone();
    let onclick = Callback::from(move |_: MouseEvent| {
        let is_updating = is_updating_ref.clone();
        spawn_local(async move {
            let is_updating = is_updating.clone();
            is_updating.set(true);
            let _ = invoke(ClientInvoke::UpdateAndRestart.as_ref(), JsValue::NULL).await;
        });
    });

    let rando: usize = (Math::floor(Math::random() * UPDATE_GIFS.len() as f64)) as usize;

    html! {
        <div class="text-white h-screen relative">
            <Header label="Update Available!" classes={classes!("text-center")}/>
            <div class="pt-4 px-8 pb-16 h-64 overflow-scroll text-sm text-center">
                <div class="flex flex-row place-content-center">
                    <iframe
                        src={format!("https://giphy.com/embed/{}", UPDATE_GIFS[rando])}
                        height="135"
                        frameBorder="0"
                        class="giphy-embed"
                    />
                </div>
                <div class="pt-4">{"Thank you for using Spyglass!"}</div>
            </div>
            <div class={classes!("fixed", "w-full", "bottom-0", "py-4", "px-8", "bg-stone-800", "z-400", "border-t-2", "border-stone-900")}>
                <div class="flex flex-row place-content-center gap-4">
                    <Btn href="https://github.com/a5huynh/spyglass/releases">
                        {"Release Notes"}
                    </Btn>
                    <Btn {onclick} disabled={*is_updating}>
                        <icons::EmojiHappyIcon animate_spin={*is_updating} classes={classes!("mr-2")}/>
                        {"Download & Update"}
                    </Btn>
                </div>
            </div>
        </div>
    }
}
