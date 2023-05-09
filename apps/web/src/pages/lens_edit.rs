use ui_components::btn::{Btn, BtnType, BtnSize};
use wasm_bindgen::{
    prelude::{wasm_bindgen, Closure},
    JsValue,
};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::{
    client::{Lens, LensAddDocType, LensAddDocument},
    AuthStatus,
};

#[wasm_bindgen(module = "/public/gapi.js")]
extern "C" {
    #[wasm_bindgen]
    pub fn init_gapi(client_id: &str, api_key: &str);

    #[wasm_bindgen(catch)]
    pub async fn create_picker(cb: &Closure<dyn Fn(JsValue, JsValue)>) -> Result<(), JsValue>;
}

pub struct CreateLensPage {
    pub lens_identifier: String,
    pub lens_data: Option<Lens>,
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
    Reload,
    SetLensData(Lens),
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

        ctx.link().send_message(Msg::Reload);

        Self {
            lens_identifier: ctx.props().lens.clone(),
            lens_data: None,
            auth_status,
            _context_listener: context_listener,
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        let new_lens = ctx.props().lens.clone();
        if self.lens_identifier != new_lens {
            self.lens_identifier = new_lens;
            ctx.link().send_message(Msg::Reload);
            true
        } else {
            false
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::AddUrl => true,
            Msg::FilePicked { token, url } => {
                let new_source = LensAddDocument {
                    url,
                    doc_type: LensAddDocType::GDrive { token },
                };

                // Add to lens
                let auth_status = self.auth_status.clone();
                let identifier = self.lens_identifier.clone();
                let link = link.clone();
                spawn_local(async move {
                    let api = auth_status.get_client();
                    if let Err(err) = api.lens_add_source(&identifier, &new_source).await {
                        log::error!("error adding lens: {}", err);
                    } else {
                        // Reload data if successful
                        link.send_message(Msg::Reload);
                    }
                });
                true
            }
            Msg::Reload => {
                let auth_status = self.auth_status.clone();
                let identifier = self.lens_identifier.clone();
                let link = link.clone();
                spawn_local(async move {
                    let api = auth_status.get_client();
                    match api.lens_retrieve(&identifier).await {
                        Ok(lens) => link.send_message(Msg::SetLensData(lens)),
                        Err(err) => log::error!("error retrieving lens: {}", err),
                    }
                });

                false
            }
            Msg::SetLensData(lens_data) => {
                self.lens_data = Some(lens_data);
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
            .lens_data
            .as_ref()
            .map(|x| x.sources.clone())
            .unwrap_or_default();

        let sources = sources
            .iter()
            .map(|x| {
                html! {
                    <div class="border-b border-neutral-600 py-4 flex flex-row items-center">
                        <div>
                            <div>{x.display_name.clone()}</div>
                            <div class="text-sm text-neutral-600 overflow-hidden">{x.url.clone()}</div>
                        </div>
                        <div class="text-base ml-auto">{x.status.clone()}</div>
                    </div>
                }
            })
            .collect::<Html>();

        html! {
            <div>
                <div class="flex flex-row items-center px-8 pt-6">
                    {if let Some(lens_data) = self.lens_data.as_ref() {
                        html! {
                            <input
                                class="border-b-2 border-neutral-600 py-3 bg-neutral-800 text-white text-2xl outline-none active:outline-none focus:outline-none caret-white"
                                type="text"
                                spellcheck="false"
                                tabindex="-1"
                                value={format!("{}", lens_data.display_name)}
                            />
                        }
                    } else {
                        html! {
                            <h2 class="bold text-xl ">{"Loading"}</h2>
                        }
                    }}
                    <Btn _type={BtnType::Success} classes="ml-auto">{"Save"}</Btn>
                </div>
                <div class="flex flex-col gap-8 px-8 py-4">
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
