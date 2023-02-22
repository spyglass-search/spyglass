use gloo::events::EventListener;
use pages::WizardStage;
use serde::{de::DeserializeOwned, Serialize};
use shared::event::ClientInvoke;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

mod components;
mod constants;
mod pages;
mod utils;

use crate::pages::{SearchPage, SettingsPage, StartupPage, UpdaterPage, WizardPage};

#[cfg(headless)]
#[wasm_bindgen(module = "/public/fixtures.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    pub async fn invoke(fn_name: &str, val: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn listen(
        event_name: &str,
        cb: &Closure<dyn Fn(JsValue)>,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "deleteDoc", catch)]
    pub async fn delete_doc(id: String) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn save_user_settings(settings: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "searchDocs", catch)]
    pub async fn search_docs(lenses: JsValue, query: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "searchLenses", catch)]
    pub async fn search_lenses(query: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn open_folder_path(path: String) -> Result<(), JsValue>;

    #[wasm_bindgen(js_name = "resizeWindow", catch)]
    pub async fn resize_window(height: f64) -> Result<(), JsValue>;

    #[wasm_bindgen]
    pub async fn network_change(is_offline: bool);

    #[wasm_bindgen(catch)]
    pub async fn recrawl_domain(domain: String) -> Result<(), JsValue>;
}

#[cfg(not(headless))]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__"], catch)]
    pub async fn invoke(fn_name: &str, val: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], catch)]
    pub async fn listen(
        event_name: &str,
        cb: &Closure<dyn Fn(JsValue)>,
    ) -> Result<JsValue, JsValue>;

}

#[cfg(not(headless))]
#[wasm_bindgen(module = "/public/glue.js")]
extern "C" {
    #[wasm_bindgen(js_name = "deleteDoc", catch)]
    pub async fn delete_doc(id: String) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn save_user_settings(settings: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "searchDocs", catch)]
    pub async fn search_docs(lenses: JsValue, query: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "searchLenses", catch)]
    pub async fn search_lenses(query: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn open_folder_path(path: String) -> Result<(), JsValue>;

    #[wasm_bindgen(js_name = "resizeWindow", catch)]
    pub async fn resize_window(height: f64) -> Result<(), JsValue>;

    #[wasm_bindgen]
    pub async fn network_change(is_offline: bool);

    #[wasm_bindgen(catch)]
    pub async fn recrawl_domain(domain: String) -> Result<(), JsValue>;
}

#[derive(Clone, Routable, PartialEq, Eq)]
pub enum Route {
    #[at("/")]
    Search,
    #[at("/settings/:tab")]
    SettingsPage { tab: pages::Tab },
    // On launch, display a little window while waiting for certain actions to finish
    // aka DB migrations.
    #[at("/startup")]
    Startup,
    #[at("/updater")]
    Updater,
    #[at("/wizard")]
    WizardRoot,
    #[at("/wizard/:stage")]
    Wizard { stage: pages::WizardStage },
}

/// Utility invoke function to handle types & proper serialization/deserialization from JS
pub async fn tauri_invoke<T: Serialize, R: DeserializeOwned>(
    fn_name: ClientInvoke,
    val: T,
) -> Result<R, String> {
    let ser = serde_wasm_bindgen::to_value(&val).expect("Unable to serialize invoke params");
    match invoke(fn_name.as_ref(), ser).await {
        Ok(results) => match serde_wasm_bindgen::from_value(results) {
            Ok(parsed) => Ok(parsed),
            Err(err) => Err(err.to_string()),
        },
        Err(e) => {
            if let Some(e) = e.as_string() {
                Err(e)
            } else {
                Err(format!("Error invoking {fn_name}"))
            }
        }
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}

#[function_component(App)]
pub fn app() -> Html {
    // Global events we're interested in

    // Detect loss of internet access
    use_effect(move || {
        let window = gloo::utils::window();

        let offline_listener = EventListener::new(&window, "offline", move |_| {
            spawn_local(async move {
                network_change(true).await;
            });
        });

        let online_listener = EventListener::new(&window, "online", move |_| {
            spawn_local(async move {
                network_change(false).await;
            });
        });

        || {
            drop(offline_listener);
            drop(online_listener);
        }
    });

    html! {
        <BrowserRouter>
            <Switch<Route> render={switch} />
        </BrowserRouter>
    }
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Search => html! { <SearchPage /> },
        Route::SettingsPage { tab } => html! { <SettingsPage tab={tab} /> },
        Route::Startup => html! { <StartupPage /> },
        Route::Updater => html! { <UpdaterPage /> },
        Route::WizardRoot => html! { <WizardPage stage={WizardStage::MenubarHelp} /> },
        Route::Wizard { stage } => html! { <WizardPage stage={stage} /> },
    }
}
