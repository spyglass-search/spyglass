use wasm_bindgen::{prelude::Closure, JsValue};
use yew::prelude::*;
use yew_router::prelude::*;

mod pages;

#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Home,
    #[at("/secure")]
    Secure,
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <pages::AskClippy /> },
        Route::Secure => html! {},
        Route::NotFound => html! {},
    }
}

pub async fn listen(_event_name: &str, _cb: &Closure<dyn Fn(JsValue)>) -> Result<JsValue, JsValue> {
    Ok(JsValue::NULL)
}

#[function_component]
fn App() -> Html {
    html! {
        <BrowserRouter>
            <Switch<Route> render={switch} />
        </BrowserRouter>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
