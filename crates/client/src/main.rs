use gloo::events::EventListener;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

mod components;
mod constants;
mod events;
mod pages;
mod utils;

use crate::pages::{SearchPage, SettingsPage, StartupPage, StatsPage, UpdaterPage, WizardPage};

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
    pub async fn delete_domain(domain: String) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn install_lens(download_url: String) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn save_user_settings(settings: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "searchDocs", catch)]
    pub async fn search_docs(lenses: JsValue, query: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "searchLenses", catch)]
    pub async fn search_lenses(query: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "openResult", catch)]
    pub async fn open(url: String) -> Result<(), JsValue>;

    #[wasm_bindgen(js_name = "resizeWindow", catch)]
    pub async fn resize_window(height: f64) -> Result<(), JsValue>;

    #[wasm_bindgen]
    pub async fn network_change(is_offline: bool);

    #[wasm_bindgen(catch)]
    pub async fn recrawl_domain(domain: String) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn toggle_plugin(name: &str) -> Result<(), JsValue>;
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
    pub async fn delete_domain(domain: String) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn install_lens(download_url: String) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn save_user_settings(settings: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "searchDocs", catch)]
    pub async fn search_docs(lenses: JsValue, query: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "searchLenses", catch)]
    pub async fn search_lenses(query: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "openResult", catch)]
    pub async fn open(url: String) -> Result<(), JsValue>;

    #[wasm_bindgen(js_name = "resizeWindow", catch)]
    pub async fn resize_window(height: f64) -> Result<(), JsValue>;

    #[wasm_bindgen]
    pub async fn network_change(is_offline: bool);

    #[wasm_bindgen(catch)]
    pub async fn recrawl_domain(domain: String) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn toggle_plugin(name: &str) -> Result<(), JsValue>;
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
    #[at("/stats")]
    Status,
    #[at("/updater")]
    Updater,
    #[at("/wizard")]
    Wizard,
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::start_app::<App>();
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
            <Switch<Route> render={Switch::render(switch)} />
        </BrowserRouter>
    }
}

fn switch(routes: &Route) -> Html {
    match routes {
        #[allow(clippy::let_unit_value)]
        Route::Search => html! { <SearchPage /> },
        #[allow(clippy::let_unit_value)]
        Route::SettingsPage { tab } => html! { <SettingsPage tab={tab.clone()} /> },
        #[allow(clippy::let_unit_value)]
        Route::Startup => html! { <StartupPage /> },
        #[allow(clippy::let_unit_value)]
        Route::Status => html! { <StatsPage /> },
        #[allow(clippy::let_unit_value)]
        Route::Updater => html! { <UpdaterPage /> },
        #[allow(clippy::let_unit_value)]
        Route::Wizard => html! { <WizardPage /> },
    }
}
