use ui_components::btn::{Btn, BtnType};
use wasm_bindgen::{
    prelude::{wasm_bindgen, Closure},
    JsValue,
};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::{client::LensSource, AuthStatus};

#[wasm_bindgen(module = "/public/gapi.js")]
extern "C" {
    #[wasm_bindgen]
    pub fn init_gapi(client_id: &str, api_key: &str);

    #[wasm_bindgen(catch)]
    pub async fn create_picker(cb: &Closure<dyn Fn(JsValue, JsValue)>) -> Result<(), JsValue>;
}

pub struct CreateLensPage {
    pub lens: String,
    pub sources: Vec<LensSource>,
    pub auth_status: AuthStatus,
    pub _context_listener: ContextHandle<AuthStatus>,
}

#[derive(Properties, PartialEq)]
pub struct CreateLensProps {
    pub lens: String,
}

pub enum Msg {
    AddUrl,
    FilePicked { token: String, url: String },
    OpenCloudFilePicker,
    UpdateContext(AuthStatus),
}

impl Component for CreateLensPage {
    type Message = Msg;
    type Properties = CreateLensProps;

    fn create(ctx: &Context<Self>) -> Self {
        // initialize gapi
        init_gapi(dotenv!("GOOGLE_CLIENT_ID"), dotenv!("GOOGLE_API_KEY"));

        let (auth_status, context_listener) = ctx
            .link()
            .context(ctx.link().callback(Msg::UpdateContext))
            .expect("No Message Context Provided");

        Self {
            lens: ctx.props().lens.clone(),
            sources: Vec::new(),
            auth_status,
            _context_listener: context_listener,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::AddUrl => {
                self.sources.push(LensSource {
                    url: "https://example.com".into(),
                    is_crawling: true,
                    access_token: None,
                });
                true
            }
            Msg::FilePicked { token, url } => {
                // let link = link.clone();
                let new_source = LensSource {
                    url,
                    is_crawling: true,
                    access_token: Some(token),
                };

                self.sources.push(new_source.clone());

                // Add to lens
                let auth_status = self.auth_status.clone();
                let lens_name = self.lens.clone();
                spawn_local(async move {
                    let api = auth_status.get_client();
                    if let Err(err) = api.lens_add_source(&lens_name, &new_source).await {
                        log::error!("erro adding lens: {}", err);
                    }
                });
                true
            }
            Msg::OpenCloudFilePicker => {
                let link = link.clone();
                spawn_local(async move {
                    let cb = Closure::wrap(Box::new(move |token: JsValue, payload: JsValue| {
                        if let (Ok(token), Ok(url)) = (
                            serde_wasm_bindgen::from_value::<String>(token),
                            serde_wasm_bindgen::from_value::<String>(payload),
                        ) {
                            link.send_message(Msg::FilePicked { token, url });
                        }
                    }) as Box<dyn Fn(JsValue, JsValue)>);

                    if let Err(err) = create_picker(&cb).await {
                        log::error!("create_picker error: {:?}", err);
                    }
                    cb.forget();
                });
                false
            }
            Msg::UpdateContext(auth_status) => {
                self.auth_status = auth_status;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let sources = self
            .sources
            .iter()
            .map(|x| {
                html! {
                    <div>
                        <div>{x.url.clone()}</div>
                        <div>{x.is_crawling}</div>
                    </div>
                }
            })
            .collect::<Html>();

        html! {
            <div>
                <div class="px-6 pt-4 flex flex-row items-center">
                    <h2 class="bold text-xl ">
                        {format!("New Lens: {}", self.lens.clone())}
                    </h2>
                    <Btn classes="ml-auto" _type={BtnType::Success}>{"Save"}</Btn>
                </div>
                <div class="flex flex-col gap-8 px-6 py-4">
                    <div class="flex flex-row gap-4">
                        <Btn onclick={link.callback(|_| Msg::AddUrl)}>{"Add data from URL"}</Btn>
                        <Btn onclick={link.callback(|_| Msg::OpenCloudFilePicker)}>{"Add data from Google Drive"}</Btn>
                    </div>
                    <div class="flex flex-col">
                        <div class="mb-2 text-sm font-semibold uppercase text-cyan-500">{"Sources"}</div>
                        <div class="flex flex-col">
                            {sources}
                        </div>
                    </div>
                </div>
            </div>
        }
    }
}
