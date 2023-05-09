#[macro_use]
extern crate dotenv_codegen;

use client::{Lens, UserData};
use gloo::utils::{history, window};
use js_sys::decode_uri_component;
use serde::{Deserialize, Serialize};
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

mod client;
mod components;
mod pages;
use components::nav::NavBar;
use pages::{lens_edit::CreateLensPage, AppPage};

use crate::{client::ApiClient, pages::search::SearchPageWrapper};

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Auth0User {
    pub name: String,
    pub email: String,
    pub picture: String,
    pub sub: String,
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AuthStatus {
    #[serde(rename(deserialize = "isAuthenticated"))]
    pub is_authenticated: bool,
    #[serde(rename(deserialize = "userProfile"))]
    pub user_profile: Option<Auth0User>,
    pub token: Option<String>,
    // Only used internally
    #[serde(skip)]
    pub user_data: Option<UserData>,
}

impl AuthStatus {
    pub fn get_client(&self) -> ApiClient {
        ApiClient::new(self.token.clone())
    }
}

#[wasm_bindgen(module = "/public/auth.js")]
extern "C" {
    #[wasm_bindgen]
    pub fn init_env(domain: &str, client_id: &str, redirect_uri: &str, audience: &str);

    #[wasm_bindgen(catch)]
    pub async fn check_login() -> Result<JsValue, JsValue>;

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
    #[at("/edit/:lens")]
    Edit { lens: String },
    #[at("/lens/:lens")]
    Search { lens: String },
    #[not_found]
    #[at("/404")]
    NotFound,
}

pub enum Msg {
    AuthenticateUser,
    CheckAuth,
    LoadLenses,
    SetSelectedLens(Lens),
    UpdateAuth(AuthStatus),
    UpdateUserData(UserData),
}

pub struct App {
    auth_status: AuthStatus,
    current_lens: Option<String>,
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        // Initialize JS env vars
        init_env(
            dotenv!("AUTH0_DOMAIN"),
            dotenv!("AUTH0_CLIENT_ID"),
            dotenv!("AUTH0_REDIRECT_URI"),
            dotenv!("AUTH0_AUDIENCE"),
        );

        // Check if user is logged in
        ctx.link().send_message(Msg::CheckAuth);

        Self {
            auth_status: AuthStatus {
                is_authenticated: false,
                user_profile: None,
                token: None,
                user_data: None,
            },
            current_lens: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();

        match msg {
            Msg::AuthenticateUser => {
                let link = link.clone();
                spawn_local(async move {
                    if let Ok(details) = handle_login_callback().await {
                        let _ = history().replace_state_with_url(
                            &JsValue::NULL,
                            "Spyglass Search",
                            Some("/"),
                        );
                        match serde_wasm_bindgen::from_value::<AuthStatus>(details) {
                            Ok(status) => link.send_message(Msg::UpdateAuth(status)),
                            Err(err) => {
                                log::error!("Unable to parse user profile: {}", err.to_string())
                            }
                        }
                    }
                });
                false
            }
            Msg::CheckAuth => {
                let link = link.clone();
                spawn_local(async move {
                    if let Ok(details) = check_login().await {
                        // Not logged in, load lenses
                        if details.is_null() {
                            link.send_message(Msg::LoadLenses);
                        } else {
                            // Logged in!
                            match serde_wasm_bindgen::from_value::<AuthStatus>(details) {
                                Ok(status) => link.send_message(Msg::UpdateAuth(status)),
                                Err(err) => {
                                    log::error!(
                                        "Unable to parse user profile: {}",
                                        err.to_string()
                                    );
                                    link.send_message(Msg::LoadLenses);
                                }
                            }
                        }
                    }
                });
                false
            }
            Msg::LoadLenses => {
                let link = link.clone();
                let auth_status = self.auth_status.clone();
                spawn_local(async move {
                    let api = auth_status.get_client();
                    if auth_status.is_authenticated {
                        log::info!("grabbing logged in user's data");
                        if let Ok(user_data) = api.get_user_data().await {
                            link.send_message(Msg::UpdateUserData(user_data));
                        }
                    } else {
                        log::info!("loading public data");
                        if let Ok(user_data) = api.get_user_data().await {
                            link.send_message(Msg::UpdateUserData(user_data));
                        }
                    }
                });
                false
            }
            Msg::SetSelectedLens(lens) => {
                self.current_lens = Some(lens.name);
                true
            }
            Msg::UpdateAuth(auth) => {
                self.auth_status = auth;
                link.send_message(Msg::LoadLenses);
                true
            }
            Msg::UpdateUserData(user_data) => {
                self.auth_status.user_data = Some(user_data);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let search = window().location().search().unwrap_or_default();
        let link = ctx.link();

        // Handle auth callbacks
        if search.contains("state=") {
            link.send_message(Msg::AuthenticateUser);
        }

        let handle_on_create_lens = {
            let link = link.clone();
            Callback::from(move |lens| {
                link.send_message_batch(vec![Msg::LoadLenses, Msg::SetSelectedLens(lens)])
            })
        };

        let switch = {
            let link = link.clone();
            move |routes: Route| match &routes {
                Route::Start => html! { <AppPage /> },
                Route::Edit { lens } => html! {
                    <AppPage>
                        <CreateLensPage lens={lens.clone()} onupdate={link.callback(|_| Msg::LoadLenses)} />
                    </AppPage>
                },
                Route::Search { lens } => {
                    let decoded_lens = if let Ok(Some(decoded)) =
                        decode_uri_component(lens).map(|x| x.as_string())
                    {
                        decoded
                    } else {
                        lens.clone()
                    };

                    html! { <AppPage><SearchPageWrapper lens={decoded_lens} /></AppPage> }
                }
                Route::NotFound => html! { <div>{"Not Found!"}</div> },
            }
        };

        html! {
            <ContextProvider<AuthStatus> context={self.auth_status.clone()}>
                <BrowserRouter>
                    <div class="text-white flex h-screen">
                        <NavBar
                            current_lens={self.current_lens.clone()}
                            // Reload lenses after a new one is created.
                            on_create_lens={handle_on_create_lens.clone()}
                            on_select_lens={link.callback(Msg::SetSelectedLens)}
                            on_edit_lens={link.callback(Msg::SetSelectedLens)}
                        />
                        <Switch<Route> render={switch} />
                    </div>
                </BrowserRouter>
            </ContextProvider<AuthStatus>>
        }
    }
}

fn main() {
    let _ = console_log::init_with_level(log::Level::Debug);
    yew::Renderer::<App>::new().render();
}
