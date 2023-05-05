#[macro_use]
extern crate dotenv_codegen;

use client::UserData;
use gloo::utils::{history, window};
use js_sys::decode_uri_component;
use serde::{Deserialize, Serialize};
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

mod client;
mod pages;
use pages::AppPage;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Auth0User {
    pub name: String,
    pub email: String,
    pub picture: String,
    pub sub: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthStatus {
    #[serde(rename(deserialize = "isAuthenticated"))]
    pub is_authenticated: bool,
    #[serde(rename(deserialize = "userProfile"))]
    pub user_profile: Option<Auth0User>,
    pub token: Option<String>,
    // Only used internall
    #[serde(skip)]
    pub user_data: Option<UserData>,
}

#[wasm_bindgen(module = "/public/auth.js")]
extern "C" {
    #[wasm_bindgen]
    pub fn init_env(domain: &str, client_id: &str, redirect_uri: &str, audience: &str);

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
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(routes: Route) -> Html {
    match &routes {
        Route::Start => html! { <AppPage /> },
        Route::Search { lens } => {
            let lens = if let Ok(Some(decoded)) = decode_uri_component(lens).map(|x| x.as_string())
            {
                decoded
            } else {
                lens.clone()
            };

            html! { <AppPage current_lens={lens} /> }
        }
        Route::NotFound => html! { <div>{"Not Found!"}</div> },
    }
}

pub async fn listen(_event_name: &str, _cb: &Closure<dyn Fn(JsValue)>) -> Result<JsValue, JsValue> {
    Ok(JsValue::NULL)
}

#[function_component]
fn App() -> Html {
    // Initialize JS env vars
    init_env(
        dotenv!("AUTH0_DOMAIN"),
        dotenv!("AUTH0_CLIENT_ID"),
        dotenv!("AUTH0_REDIRECT_URI"),
        dotenv!("AUTH0_AUDIENCE"),
    );

    let is_logged_in = use_state_eq(|| false);
    let auth_status: UseStateHandle<AuthStatus> = use_state_eq(|| AuthStatus {
        is_authenticated: false,
        user_profile: None,
        token: None,
        user_data: None,
    });

    let search = window().location().search().unwrap_or_default();

    // Reload lenses if auth_status changes
    {
        let auth_status_handle = auth_status.clone();
        use_effect_with_deps(
            move |_| {
                if auth_status_handle.user_data.is_some() {
                    return;
                }

                let mut updated = (*auth_status_handle).clone();
                spawn_local(async move {
                    if let Some(token) = &auth_status_handle.token {
                        log::info!("grabbing logged in user's data");
                        if let Ok(user_data) = client::get_user_data(Some(token.to_owned())).await {
                            updated.user_data = Some(user_data);
                        }
                    } else {
                        log::info!("loading public data");
                        if let Ok(user_data) = client::get_user_data(None).await {
                            updated.user_data = Some(user_data);
                        }
                    }

                    auth_status_handle.set(updated);
                });
            },
            is_logged_in.clone(),
        );
    }

    // Handle auth callbacks
    if search.contains("state=") {
        let auth_status_handle = auth_status.clone();
        let is_logged_in_handle = is_logged_in.clone();
        spawn_local(async move {
            if let Ok(details) = handle_login_callback().await {
                let _ =
                    history().replace_state_with_url(&JsValue::NULL, "Spyglass Search", Some("/"));
                match serde_wasm_bindgen::from_value::<AuthStatus>(details) {
                    Ok(value) => {
                        is_logged_in_handle.set(true);
                        auth_status_handle.set(value)
                    },
                    Err(err) => log::error!("Unable to parse user profile: {}", err.to_string()),
                }
            }
        });
    }

    html! {
        <ContextProvider<AuthStatus> context={(*auth_status).clone()}>
            <BrowserRouter>
                <Switch<Route> render={switch} />
            </BrowserRouter>
        </ContextProvider<AuthStatus>>

    }
}

fn main() {
    let _ = console_log::init_with_level(log::Level::Debug);
    yew::Renderer::<App>::new().render();
}
