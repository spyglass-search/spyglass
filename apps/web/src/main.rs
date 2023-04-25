use wasm_bindgen::{prelude::Closure, JsValue};
use yew::prelude::*;
use yew_router::prelude::*;

mod client;
mod constants;
mod pages;
use pages::AppPage;

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
        Route::Start => html! { <Redirect<Route> to={Route::Search { lens: "yc".into() }} /> },
        Route::Search { lens } => html! { <AppPage lens={lens.clone()} /> },
        Route::NotFound => html! { <div>{"Not Found!"}</div> },
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
    let _ = console_log::init_with_level(log::Level::Debug);
    yew::Renderer::<App>::new().render();
}
