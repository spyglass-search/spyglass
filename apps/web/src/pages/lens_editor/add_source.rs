use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};
use ui_components::btn::{Btn, BtnSize, BtnType};
use ui_components::icons;
use wasm_bindgen::{prelude::*, JsValue};
use web_sys::HtmlInputElement;
use yew::{html::Scope, platform::spawn_local, prelude::*};

use crate::{
    client::{ApiError, LensAddDocType, LensAddDocument},
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
    adding_in_progress: bool,
    auth_status: AuthStatus,
    selected_tab: AddSourceTabs,
    _context_listener: ContextHandle<AuthStatus>,
    _feed_input_ref: NodeRef,
    _url_input_ref: NodeRef,
    _url_crawl_ref: NodeRef,
}

pub enum Msg {
    AddUrl,
    AddFeed,
    ChangeToTab(AddSourceTabs),
    EmitError(String),
    EmitUpdate,
    FilePicked { token: String, url: String },
    OpenCloudFilePicker,
    UpdateContext(AuthStatus),
}

#[derive(Properties, PartialEq)]
pub struct AddSourceComponentProps {
    pub lens_identifier: String,
    #[prop_or_default]
    pub on_error: Callback<String>,
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
            adding_in_progress: false,
            auth_status,
            selected_tab: AddSourceTabs::Website,
            _context_listener: context_listener,
            _feed_input_ref: NodeRef::default(),
            _url_input_ref: NodeRef::default(),
            _url_crawl_ref: NodeRef::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        let props = ctx.props();

        match msg {
            Msg::AddFeed => {
                if let Some(feed_input) = self._feed_input_ref.cast::<HtmlInputElement>() {
                    let url = feed_input.value();
                    let url = match url::Url::parse(&url) {
                        Ok(url) => url,
                        Err(_) => {
                            link.send_message(Msg::EmitError("Invalid URL".into()));
                            return false;
                        }
                    };

                    let source = LensAddDocument {
                        url: url.to_string(),
                        doc_type: LensAddDocType::RssFeed,
                    };

                    self.adding_in_progress = true;
                    self.add_source(&props.lens_identifier, source, link, false);
                }
                false
            }
            Msg::AddUrl => {
                if let (Some(url_input), Some(crawl_checkbox)) = (
                    self._url_input_ref.cast::<HtmlInputElement>(),
                    self._url_crawl_ref.cast::<HtmlInputElement>(),
                ) {
                    let url = url_input.value();
                    let is_crawl = crawl_checkbox.checked();

                    let url = match url::Url::parse(&url) {
                        Ok(url) => url,
                        Err(_) => {
                            link.send_message(Msg::EmitError("Invalid URL".into()));
                            return false;
                        }
                    };

                    let new_source = LensAddDocument {
                        url: url.to_string(),
                        doc_type: LensAddDocType::WebUrl {
                            include_all_suburls: is_crawl,
                        },
                    };

                    let link = link.clone();
                    self.adding_in_progress = true;
                    self.add_source(&props.lens_identifier, new_source, &link, true);
                }

                true
            }
            Msg::ChangeToTab(new_tab) => {
                self.selected_tab = new_tab;
                true
            }
            Msg::EmitError(msg) => {
                self.adding_in_progress = false;
                props.on_error.emit(msg);
                true
            }
            Msg::EmitUpdate => {
                self.adding_in_progress = false;
                props.on_update.emit(());
                true
            }
            Msg::FilePicked { token, url } => {
                let new_source = LensAddDocument {
                    url,
                    doc_type: LensAddDocType::GDrive { token },
                };

                let link = link.clone();
                self.adding_in_progress = true;
                self.add_source(&props.lens_identifier, new_source, &link, false);
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
                    <Btn
                        size={BtnSize::Sm}
                        _type={BtnType::Primary}
                        onclick={link.callback(|_| Msg::AddUrl)}
                        disabled={self.adding_in_progress}
                    >
                        {if self.adding_in_progress {
                            html! {
                                <icons::RefreshIcon
                                    width="w-4"
                                    height="h-4"
                                    animate_spin={self.adding_in_progress}
                                />
                            }
                        } else {
                            html! { <div>{"Fetch"}</div> }
                        }}
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
                        ref={self._feed_input_ref.clone()}
                        type="text"
                        class="rounded p-2 text-sm text-neutral-800 flex-grow"
                        placeholder="https://example.com/feed.rss"
                    />
                    <Btn
                        disabled={self.adding_in_progress}
                        size={BtnSize::Sm}
                        _type={BtnType::Primary}
                        onclick={link.callback(|_| Msg::AddFeed)}>
                        {if self.adding_in_progress {
                            html! {
                                <icons::RefreshIcon
                                    width="w-4"
                                    height="h-4"
                                    animate_spin={self.adding_in_progress}
                                />
                            }
                        } else {
                            html! { <div>{"Add Podcast"}</div> }
                        }}
                    </Btn>
                </div>
            </div>
        }
    }

    fn view_gdrive_tab(&self, link: &Scope<AddSourceComponent>) -> Html {
        html! {
            <div>
                <Btn
                    _type={BtnType::Primary}
                    disabled={self.adding_in_progress}
                    onclick={link.callback(|_| Msg::OpenCloudFilePicker)}
                    size={BtnSize::Sm}
                >
                    {if self.adding_in_progress {
                        html! {
                            <icons::RefreshIcon
                                width="w-4"
                                height="h-4"
                                animate_spin={self.adding_in_progress}
                            />
                        }
                    } else {
                        html! { <div>{"Select file from Google Drive"}</div> }
                    }}
                </Btn>
            </div>
        }
    }

    fn add_source(
        &self,
        lens: &str,
        source: LensAddDocument,
        link: &Scope<AddSourceComponent>,
        validate: bool,
    ) {
        let auth_status = self.auth_status.clone();
        let link: Scope<AddSourceComponent> = link.clone();
        let lens = lens.to_string();
        spawn_local(async move {
            let api = auth_status.get_client();

            let is_valid = if validate {
                match api.validate_lens_source(&lens, &source).await {
                    Ok(response) => {
                        if response.is_valid {
                            true
                        } else {
                            let error_msg = response
                                .validation_msg
                                .unwrap_or("Unknown error adding URL".to_string());
                            link.send_message(Msg::EmitError(error_msg));
                            false
                        }
                    }
                    Err(error) => {
                        link.send_message(Msg::EmitError(error.to_string()));
                        false
                    }
                }
            } else {
                true
            };

            if is_valid {
                match api.lens_add_source(&lens, &source).await {
                    Ok(_) => link.send_message(Msg::EmitUpdate),
                    Err(ApiError::ClientError(msg)) => {
                        link.send_message(Msg::EmitError(msg.message))
                    }
                    Err(err) => link.send_message(Msg::EmitError(err.to_string())),
                }
            }
        });
    }
}
