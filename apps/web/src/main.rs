use gloo::utils::{window, history};
use serde::{Deserialize, Serialize};
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

mod client;
mod constants;
mod pages;
use pages::AppPage;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Auth0User {
    pub name: String,
    pub email: String,
    pub picture: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Auth0Status {
    #[serde(rename(deserialize = "isAuthenticated"))]
    pub is_authenticated: bool,
    #[serde(rename(deserialize = "userProfile"))]
    pub user_profile: Option<Auth0User>
}

#[wasm_bindgen(module = "/public/auth.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    pub async fn auth0_login() -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn auth0_logout() -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    pub async fn handle_login_callback() -> Result<JsValue, JsValue>;
}

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Start,
    #[at("/lens/:lens")]
    Search { lens: String },
    // #[at("/library")]
    // MyLibrary,
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(routes: Route) -> Html {
    match &routes {
        Route::Start => html! { <AppPage lens={"yc".clone()} /> },
        Route::Search { lens } => html! { <AppPage lens={lens.clone()} /> },
        Route::NotFound => html! { <div>{"Not Found!"}</div> },
    }
}

pub async fn listen(_event_name: &str, _cb: &Closure<dyn Fn(JsValue)>) -> Result<JsValue, JsValue> {
    Ok(JsValue::NULL)
}

#[function_component]
fn App() -> Html {
    let auth_status: UseStateHandle<Auth0Status> = use_state_eq(|| Auth0Status { is_authenticated: false, user_profile: None });
    let search = window().location().search().unwrap_or_default();

    if search.contains("state=") {
        log::info!("handling auth callback");
        let auth_status_handle = auth_status.clone();
        spawn_local(async move {
            if let Ok(details) = handle_login_callback().await {
                let _ = history().replace_state_with_url(&JsValue::NULL, "Spyglass Search", Some("/"));
                if let Ok(value) = serde_wasm_bindgen::from_value(details) {
                    auth_status_handle.set(value);
                }
            }
        });
    }

    html! {
        <ContextProvider<Auth0Status> context={(*auth_status).clone()}>
            <BrowserRouter>
                <Switch<Route> render={switch} />
            </BrowserRouter>
        </ContextProvider<Auth0Status>>
    }
}

fn main() {
    let _ = console_log::init_with_level(log::Level::Debug);
    yew::Renderer::<App>::new().render();
}
