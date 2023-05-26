use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};
use ui_components::btn::{Btn, BtnSize, BtnType};
use wasm_bindgen::{prelude::*, JsValue};
use web_sys::HtmlInputElement;
use yew::{html::Scope, platform::spawn_local, prelude::*};

use crate::{
    client::{ApiClient, ApiError, LensAddDocType, LensAddDocument},
    AuthStatus,
};

#[wasm_bindgen(module = "/public/gapi.js")]
extern "C" {
    #[wasm_bindgen]
    pub fn init_gapi(client_id: &str, api_key: &str);

    #[wasm_bindgen(catch)]
    pub async fn create_picker(cb: &Closure<dyn Fn(JsValue, JsValue)>) -> Result<(), JsValue>;
}

#[derive(Clone, EnumIter, Display, PartialEq, Eq)]
pub enum AddSourceTabs {
    Website,
    Podcast,
    GDrive,
}

pub struct AddSourceComponent {
    auth_status: AuthStatus,
    error_msg: Option<String>,
    selected_tab: AddSourceTabs,
    _context_listener: ContextHandle<AuthStatus>,
    _url_input_ref: NodeRef,
    _url_crawl_ref: NodeRef,
}

pub enum Msg {
    AddUrl { include_all: bool },
    ChangeToTab(AddSourceTabs),
    EmitUpdate,
    FilePicked { token: String, url: String },
    SetError(String),
    OpenCloudFilePicker,
    UpdateContext(AuthStatus),
}

#[derive(Properties, PartialEq)]
pub struct AddSourceComponentProps {
    pub lens_identifier: String,
    #[prop_or_default]
    pub on_update: Callback<()>,
}

impl Component for AddSourceComponent {
    type Message = Msg;
    type Properties = AddSourceComponentProps;

    fn create(ctx: &Context<Self>) -> Self {
        // initialize gapi
        init_gapi(dotenv!("GOOGLE_CLIENT_ID"), dotenv!("GOOGLE_API_KEY"));
        // Connect to context for auth details
        let (auth_status, context_listener) = ctx
            .link()
            .context(ctx.link().callback(Msg::UpdateContext))
            .expect("No Message Context Provided");

        Self {
            auth_status,
            error_msg: None,
            selected_tab: AddSourceTabs::Website,
            _context_listener: context_listener,
            _url_input_ref: NodeRef::default(),
            _url_crawl_ref: NodeRef::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        let props = ctx.props();

        match msg {
            Msg::AddUrl { include_all } => {
                if let Some(node) = self._url_input_ref.cast::<HtmlInputElement>() {
                    let url = node.value();

                    if let Err(_err) = url::Url::parse(&url) {
                        link.send_message(Msg::SetError("Invalid Url".to_string()));
                    } else {
                        let new_source = LensAddDocument {
                            url,
                            doc_type: LensAddDocType::WebUrl {
                                include_all_suburls: include_all,
                            },
                        };
                        // Add to lens
                        let auth_status = self.auth_status.clone();
                        let identifier = props.lens_identifier.clone();
                        let link = link.clone();

                        if include_all {
                            spawn_local(async move {
                                let api = auth_status.get_client();
                                match api.validate_lens_source(&identifier, &new_source).await {
                                    Ok(response) => {
                                        if response.is_valid {
                                            node.set_value("");
                                            add_lens_source(&api, &new_source, &identifier, link)
                                                .await;
                                        } else if let Some(error_msg) = response.validation_msg {
                                            link.send_message(Msg::SetError(error_msg));
                                        } else {
                                            link.send_message(Msg::SetError(
                                                "Unknown error adding url".to_string(),
                                            ));
                                        }
                                    }
                                    Err(error) => {
                                        log::error!("Unknown error adding url {:?}", error);
                                        link.send_message(Msg::SetError(
                                            "Unknown error adding url".to_string(),
                                        ));
                                    }
                                }
                            })
                        } else {
                            node.set_value("");
                            spawn_local(async move {
                                let api = auth_status.get_client();
                                add_lens_source(&api, &new_source, &identifier, link).await;
                            });
                        }
                    }
                }
                true
            }
            Msg::ChangeToTab(new_tab) => {
                self.selected_tab = new_tab;
                true
            }
            Msg::EmitUpdate => {
                props.on_update.emit(());
                false
            }
            Msg::FilePicked { token, url } => {
                let new_source = LensAddDocument {
                    url,
                    doc_type: LensAddDocType::GDrive { token },
                };

                // Add to lens
                let auth_status = self.auth_status.clone();
                let identifier = props.lens_identifier.clone();
                let link = link.clone();
                spawn_local(async move {
                    let api = auth_status.get_client();
                    if let Err(err) = api.lens_add_source(&identifier, &new_source).await {
                        log::error!("error adding gdrive source: {err}");
                    } else {
                        link.send_message(Msg::EmitUpdate);
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
            _ => false,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let tab_styles = classes!(
            "border-b-2",
            "px-4",
            "pb-2",
            "-mb-[2px]",
            "cursor-pointer",
            "hover:text-cyan-500",
            "hover:border-cyan-500"
        );
        let tabs = AddSourceTabs::iter()
            .map(|tab| {
                let mut styles = tab_styles.clone();
                if self.selected_tab == tab {
                    styles.extend(vec!["text-cyan-500", "border-cyan-500"]);
                } else {
                    styles.extend(vec!["text-white", "border-neutral-700"]);
                }

                let action = {
                    let new_tab = tab.clone();
                    link.callback(move |_| Msg::ChangeToTab(new_tab.clone()))
                };

                html! { <li onclick={action} class={styles.clone()}>{tab}</li> }
            })
            .collect::<Html>();

        html! {
            <div class="flex flex-col gap-4">
                {if let Some(msg) = &self.error_msg {
                    html! {
                        <div class="bg-red-300 border border-red-700 text-red-700 rounded-lg text-sm p-2 font-bold">
                            {msg}
                        </div>
                    }
                } else { html! {} }}
                <div>
                    <ul class="text-sm flex flex-row gap-8 w-full border-b-2 border-neutral-700 mb-4">
                        {tabs}
                    </ul>
                    <div class="px-2 md:px-8 py-2">
                        {match self.selected_tab {
                            AddSourceTabs::Website => self.view_website_tab(link),
                            AddSourceTabs::Podcast => self.view_podcast_tab(link),
                            AddSourceTabs::GDrive => self.view_gdrive_tab(link),
                        }}
                    </div>
                </div>
            </div>
        }
    }
}

impl AddSourceComponent {
    fn view_website_tab(&self, link: &Scope<AddSourceComponent>) -> Html {
        html! {
            <div>
                <div class="text-xs text-neutral-400 pb-2">
                    {"Add a single page or all pages from a website"}
                </div>
                <div class="flex flex-row gap-4 items-center">
                    <input
                        ref={self._url_input_ref.clone()}
                        type="text"
                        class="rounded p-2 text-sm text-neutral-800 flex-grow"
                        placeholder="https://example.com"
                    />
                    <div>
                        <label class="flex flex-row gap-2 text-sm">
                            <input
                                ref={self._url_crawl_ref.clone()}
                                type="checkbox"
                            />
                            {"Add all pages"}
                        </label>
                    </div>
                    <Btn size={BtnSize::Sm} _type={BtnType::Primary} onclick={link.callback(|_| Msg::AddUrl {include_all: false})}>
                        {"Fetch"}
                    </Btn>
                </div>
            </div>
        }
    }

    fn view_podcast_tab(&self, link: &Scope<AddSourceComponent>) -> Html {
        html! {
            <div>
                <div class="text-xs text-neutral-400 pb-2">
                    {"Add episodes from a podcast feed"}
                </div>
                <div class="flex flex-row gap-4 items-center">
                    <input
                        ref={self._url_input_ref.clone()}
                        type="text"
                        class="rounded p-2 text-sm text-neutral-800 flex-grow"
                        placeholder="https://example.com/feed.rss"
                    />
                    <Btn size={BtnSize::Sm} _type={BtnType::Primary} onclick={link.callback(|_| Msg::AddUrl {include_all: false})}>
                        {"Add Podcast"}
                    </Btn>
                </div>
            </div>
        }
    }

    fn view_gdrive_tab(&self, link: &Scope<AddSourceComponent>) -> Html {
        html! {
            <div>
                <Btn onclick={link.callback(|_| Msg::OpenCloudFilePicker)} _type={BtnType::Primary}>
                    {"Select file from Google Drive"}
                </Btn>
            </div>
        }
    }
}

async fn add_lens_source(
    api: &ApiClient,
    new_source: &LensAddDocument,
    identifier: &str,
    link: Scope<AddSourceComponent>,
) {
    if let Err(err) = api.lens_add_source(identifier, new_source).await {
        log::error!("error adding url source: {err}");
        match err {
            ApiError::ClientError(msg) => link.send_message(Msg::SetError(msg.message)),
            _ => link.send_message(Msg::SetError(err.to_string())),
        };
    } else {
        link.send_message(Msg::EmitUpdate);
    }
}
