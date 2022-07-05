use gloo::events::EventListener;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

mod components;
mod constants;
mod events;
mod pages;

use crate::pages::{LensManagerPage, SearchPage, StatsPage};

#[wasm_bindgen(module = "/public/glue.js")]
extern "C" {
    #[wasm_bindgen(js_name = "deleteDoc", catch)]
    pub async fn delete_doc(id: String) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn install_lens(download_url: String) -> Result<(), JsValue>;

    #[wasm_bindgen(js_name = "listInstalledLenses", catch)]
    pub async fn list_installed_lenses() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "listInstallableLenses", catch)]
    pub async fn list_installable_lenses() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "searchDocs", catch)]
    pub async fn search_docs(lenses: JsValue, query: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "searchLenses", catch)]
    pub async fn search_lenses(query: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "onClearSearch")]
    pub async fn on_clear_search(callback: &Closure<dyn Fn()>);

    #[wasm_bindgen(js_name = "onFocus")]
    pub async fn on_focus(callback: &Closure<dyn Fn()>);

    #[wasm_bindgen(js_name = "onRefreshResults")]
    pub async fn on_refresh_results(callback: &Closure<dyn Fn()>);

    #[wasm_bindgen]
    pub async fn on_refresh_lens_manager(callback: &Closure<dyn Fn()>);

    #[wasm_bindgen(js_name = "openResult", catch)]
    pub async fn open(url: String) -> Result<(), JsValue>;

    #[wasm_bindgen(js_name = "openLensFolder", catch)]
    pub async fn open_lens_folder() -> Result<(), JsValue>;

    #[wasm_bindgen(js_name = "escape", catch)]
    pub async fn escape() -> Result<(), JsValue>;

    #[wasm_bindgen(js_name = "resizeWindow", catch)]
    pub async fn resize_window(height: f64) -> Result<(), JsValue>;

    #[wasm_bindgen(js_name = "crawlStats", catch)]
    pub async fn crawl_stats() -> Result<JsValue, JsValue>;

    #[wasm_bindgen]
    pub async fn network_change(is_offline: bool);

    #[wasm_bindgen(catch)]
    pub async fn recrawl_domain(domain: String) -> Result<(), JsValue>;
}

#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Search,
    #[at("/settings/lens")]
    LensManager,
    #[at("/stats")]
    Status,
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
        Route::LensManager => html! { <LensManagerPage /> },
        Route::Search => html! { <SearchPage /> },
        Route::Status => html! { <StatsPage /> },
    }
}
