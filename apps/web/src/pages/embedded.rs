use crate::{
    client::{ApiClient, ApiError, Lens},
    pages::{chat::ChatPage, search::SearchPage},
    schema::EmbeddedPromptStyle,
};

use wasm_bindgen_futures::spawn_local;
use yew::{html::Scope, prelude::*};
use yew_router::prelude::*;

pub enum Msg {
    Reload,
    SetLensData(Box<Lens>),
}

#[derive(Properties, PartialEq)]
pub struct EmbeddedPageProps {
    pub lens: String,
    pub session_uuid: String,
}

/// The embedded page is used to toggle between the different types
/// of embedded pages based on the lens configuration. To
/// speed up load time the lens configuration is passed into the
/// search page. This allows the search page to skip the need for
/// an additional http request to render
pub struct EmbeddedPage {
    lens_identifier: String,
    lens_data: Option<Lens>,
    session_uuid: String,
}

impl Component for EmbeddedPage {
    type Message = Msg;
    type Properties = EmbeddedPageProps;

    fn create(ctx: &yew::Context<Self>) -> Self {
        let props = ctx.props();

        ctx.link().send_message(Msg::Reload);
        Self {
            lens_data: None,
            lens_identifier: props.lens.clone(),
            session_uuid: props.session_uuid.clone(),
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        let new_lens = ctx.props().lens.clone();
        let lens_changed = self.lens_identifier != new_lens;

        if lens_changed {
            ctx.link().send_message(Msg::Reload);
            true
        } else {
            false
        }
    }

    fn update(&mut self, ctx: &yew::Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::Reload => {
                self.reload(link);
                true
            }
            Msg::SetLensData(data) => {
                self.lens_data = Some(*data);
                true
            }
        }
    }

    fn view(&self, _ctx: &yew::Context<Self>) -> yew::Html {
        if let Some(lens) = &self.lens_data {
            let prompt_style = lens
                .embedded_configuration
                .as_ref()
                .map(|cfg| cfg.prompt_style.clone())
                .unwrap_or(EmbeddedPromptStyle::Research);
            match prompt_style {
                EmbeddedPromptStyle::Research => {
                    html! {
                        <SearchPage lens={self.lens_identifier.clone()} lens_data={Some(lens.clone())} session_uuid={self.session_uuid.clone()} embedded=true/>
                    }
                }
                EmbeddedPromptStyle::Chat => {
                    html! {
                        <ChatPage lens={self.lens_identifier.clone()} lens_data={Some(lens.clone())} session_uuid={self.session_uuid.clone()}/>
                    }
                }
            }
        } else {
            html! {}
        }
    }
}

impl EmbeddedPage {
    // Reload the lens configuration
    fn reload(&mut self, link: &Scope<EmbeddedPage>) {
        let identifier = self.lens_identifier.clone();
        let link = link.clone();
        spawn_local(async move {
            let api = ApiClient::new(None, true);
            match api.lens_retrieve(&identifier).await {
                Ok(lens) => link.send_message(Msg::SetLensData(Box::new(lens))),
                Err(ApiError::ClientError(msg)) => {
                    log::error!("Got error! {:?}", msg);
                    // Unauthorized
                    if msg.code == 400 {
                        let navi = link.navigator().expect("No navigator");
                        navi.push(&crate::Route::Start);
                    }
                    log::error!("error retrieving lens: {msg}");
                }
                Err(err) => log::error!("error retrieving lens: {}", err),
            }
        });
    }
}
