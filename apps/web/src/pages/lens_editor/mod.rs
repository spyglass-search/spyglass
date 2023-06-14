use gloo::timers::callback::{Interval, Timeout};
use strum::IntoEnumIterator;
use ui_components::{
    btn::{Btn, BtnSize, BtnType},
    icons,
    results::Paginator,
};
use wasm_bindgen::{prelude::wasm_bindgen, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_router::scope_ext::RouterScopeExt;

use crate::{
    client::{ApiError, Lens, LensDocType, LensSource},
    download_file,
    schema::{GetLensSourceResponse, LensSourceQueryFilter},
    AuthStatus,
};

mod add_source;
use add_source::AddSourceComponent;

const QUERY_DEBOUNCE_MS: u32 = 1_000;
const REFRESH_INTERVAL_MS: u32 = 5_000;

const DOWNLOAD_PREFIX: &str = "https://search.spyglass.fyi/lens";

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "clearTimeout")]
    fn clear_timeout(handle: JsValue);
}

#[derive(Clone, PartialEq)]
pub struct LensSourcePaginator {
    page: usize,
    num_items: usize,
    num_pages: usize,
}

pub struct CreateLensPage {
    pub error_msg: Option<String>,
    pub lens_identifier: String,
    pub lens_data: Option<Lens>,

    pub source_filter: LensSourceQueryFilter,
    pub lens_sources: Option<Vec<LensSource>>,
    pub lens_source_paginator: Option<LensSourcePaginator>,

    pub is_loading_lens_sources: bool,
    pub is_saving_name: bool,

    pub auth_status: AuthStatus,
    pub add_url_error: Option<String>,

    pub _refresh_interval: Option<Interval>,
    pub _context_listener: ContextHandle<AuthStatus>,
    pub _query_debounce: Option<JsValue>,
    pub _name_input_ref: NodeRef,
}

#[derive(Properties, PartialEq)]
pub struct CreateLensProps {
    pub lens: String,
}

pub enum Msg {
    ClearError,
    DeleteLensSource(LensSource),
    Reload,
    ReloadCurrentSources,
    ReloadSources {
        page: usize,
        filter: LensSourceQueryFilter,
    },
    Save {
        display_name: String,
    },
    SaveDone,
    SetError(String),
    SetFilter(LensSourceQueryFilter),
    SetLensData(Lens),
    SetLensSources(GetLensSourceResponse),
    UpdateContext(AuthStatus),
    UpdateDisplayName,
}

impl Component for CreateLensPage {
    type Message = Msg;
    type Properties = CreateLensProps;

    fn create(ctx: &Context<Self>) -> Self {
        let (auth_status, context_listener) = ctx
            .link()
            .context(ctx.link().callback(Msg::UpdateContext))
            .expect("No Message Context Provided");

        ctx.link().send_message_batch(vec![
            Msg::Reload,
            Msg::ReloadSources {
                page: 0,
                filter: LensSourceQueryFilter::default(),
            },
        ]);

        Self {
            error_msg: None,
            lens_identifier: ctx.props().lens.clone(),
            lens_data: None,
            lens_sources: None,
            lens_source_paginator: None,
            source_filter: LensSourceQueryFilter::default(),
            is_saving_name: false,
            is_loading_lens_sources: false,
            auth_status,
            add_url_error: None,
            _refresh_interval: None,
            _context_listener: context_listener,
            _query_debounce: None,
            _name_input_ref: NodeRef::default(),
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        let new_lens = ctx.props().lens.clone();
        if self.lens_identifier != new_lens {
            self.lens_identifier = new_lens;

            let reload_msg = if let Some(paginator) = &self.lens_source_paginator {
                Msg::ReloadSources {
                    page: paginator.page,
                    filter: self.source_filter,
                }
            } else {
                Msg::ReloadSources {
                    page: 0,
                    filter: LensSourceQueryFilter::All,
                }
            };

            ctx.link().send_message_batch(vec![Msg::Reload, reload_msg]);
            true
        } else {
            false
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::ClearError => {
                self.error_msg = None;
                true
            }
            Msg::DeleteLensSource(source) => {
                // Add to lens
                let auth_status = self.auth_status.clone();
                let identifier = self.lens_identifier.clone();
                let link = link.clone();
                let page = self
                    .lens_source_paginator
                    .as_ref()
                    .map(|x| x.page)
                    .unwrap_or(0);
                let filter = self.source_filter;
                spawn_local(async move {
                    let api = auth_status.get_client();
                    match api.delete_lens_source(&identifier, &source.doc_uuid).await {
                        Ok(_) => link.send_message(Msg::ReloadSources { page, filter }),
                        Err(err) => {
                            log::error!("Error deleting source: {err}");
                            link.send_message(Msg::SetError(err.to_string()));
                        }
                    }
                });
                false
            }
            Msg::Reload => {
                let auth_status = self.auth_status.clone();
                let identifier = self.lens_identifier.clone();
                let link = link.clone();
                spawn_local(async move {
                    let api = auth_status.get_client();
                    match api.lens_retrieve(&identifier).await {
                        Ok(lens) => link.send_message_batch(vec![
                            Msg::SetLensData(lens),
                            Msg::ReloadCurrentSources,
                        ]),
                        Err(ApiError::ClientError(msg)) => {
                            // Unauthorized
                            if msg.code == 400 {
                                let navi = link.navigator().expect("No navigator");
                                navi.push(&crate::Route::Start);
                            }

                            log::error!("error retrieving lens: {msg}");
                        }
                        Err(err) => log::error!("error retrieving lens: {err}"),
                    }
                });

                false
            }
            Msg::ReloadCurrentSources => {
                if let Some(paginator) = &self.lens_source_paginator {
                    link.send_message(Msg::ReloadSources {
                        page: paginator.page,
                        filter: self.source_filter,
                    });
                }
                false
            }
            Msg::ReloadSources { page, filter } => {
                let auth_status = self.auth_status.clone();
                let identifier = self.lens_identifier.clone();
                let link = link.clone();
                self.is_loading_lens_sources = true;
                spawn_local(async move {
                    let api: crate::client::ApiClient = auth_status.get_client();
                    match api.lens_retrieve_sources(&identifier, page, filter).await {
                        Ok(lens) => link.send_message(Msg::SetLensSources(lens)),
                        Err(ApiError::ClientError(msg)) => {
                            // Unauthorized
                            if msg.code == 400 {
                                let navi = link.navigator().expect("No navigator");
                                navi.push(&crate::Route::Start);
                            }

                            log::error!("error retrieving lens: {msg}");
                        }
                        Err(err) => log::error!("error retrieving lens: {err}"),
                    }
                });

                true
            }
            Msg::Save { display_name } => {
                if let Some(lens_data) = &mut self.lens_data {
                    let auth_status = self.auth_status.clone();
                    let identifier = self.lens_identifier.clone();
                    let link = link.clone();
                    self.is_saving_name = true;
                    lens_data.display_name = display_name.clone();
                    spawn_local(async move {
                        let api = auth_status.get_client();
                        if api.lens_update(&identifier, &display_name).await.is_ok() {
                            link.send_message(Msg::SaveDone);
                        } else {
                            link.send_message(Msg::Reload);
                        }
                    });
                }
                true
            }
            Msg::SaveDone => {
                self.is_saving_name = false;
                true
            }
            Msg::SetError(err) => {
                self.error_msg = Some(err);
                true
            }
            Msg::SetFilter(filter) => {
                self.source_filter = filter;
                if let Some(paginator) = &self.lens_source_paginator {
                    link.send_message(Msg::ReloadSources {
                        page: paginator.page,
                        filter: self.source_filter,
                    });
                }
                true
            }
            Msg::SetLensData(lens_data) => {
                self.lens_data = Some(lens_data);
                true
            }
            Msg::SetLensSources(sources) => {
                self.is_loading_lens_sources = false;
                self.lens_source_paginator = Some(LensSourcePaginator {
                    page: sources.page,
                    num_items: sources.num_items,
                    num_pages: sources.num_pages,
                });

                let has_processing = sources.results.iter().any(|x| x.status == "Processing");

                if has_processing && self._refresh_interval.is_none() {
                    let link = link.clone();
                    let interval = Interval::new(REFRESH_INTERVAL_MS, move || {
                        link.send_message(Msg::ReloadCurrentSources);
                    });

                    self._refresh_interval = Some(interval);
                } else if !has_processing {
                    self._refresh_interval = None;
                }

                self.lens_sources = Some(sources.results);
                true
            }
            Msg::UpdateContext(auth_status) => {
                self.auth_status = auth_status;
                let page = self
                    .lens_source_paginator
                    .as_ref()
                    .map(|x| x.page)
                    .unwrap_or(0);
                link.send_message_batch(vec![
                    Msg::Reload,
                    Msg::ReloadSources {
                        page,
                        filter: self.source_filter,
                    },
                ]);
                true
            }
            Msg::UpdateDisplayName => {
                if let Some(timeout_id) = &self._query_debounce {
                    clear_timeout(timeout_id.clone());
                    self._query_debounce = None;
                }

                {
                    if let Some(node) = self._name_input_ref.cast::<HtmlInputElement>() {
                        let display_name = node.value();
                        let link = link.clone();
                        let handle = Timeout::new(QUERY_DEBOUNCE_MS, move || {
                            link.send_message(Msg::Save { display_name })
                        });

                        let id = handle.forget();
                        self._query_debounce = Some(id);
                    }
                }

                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        html! {
            <div class="flex flex-col px-8 pt-6 gap-4">
                {if let Some(msg) = &self.error_msg {
                    html! {
                        <div class="bg-red-100 border border-red-400 text-red-700 p-2 text-sm rounded-lg font-semibold relative" role="alert">
                            <div>{msg}</div>
                            <span class="absolute top-0 bottom-0 right-0 p-2">
                                <svg
                                    onclick={link.callback(|_| Msg::ClearError)}
                                    class="fill-current h-5 w-5 text-red-500"
                                    role="button"
                                    xmlns="http://www.w3.org/2000/svg"
                                    viewBox="0 0 20 20"
                                >
                                    <path d="M14.348 14.849a1.2 1.2 0 0 1-1.697 0L10 11.819l-2.651 3.029a1.2 1.2 0 1 1-1.697-1.697l2.758-3.15-2.759-3.152a1.2 1.2 0 1 1 1.697-1.697L10 8.183l2.651-3.031a1.2 1.2 0 1 1 1.697 1.697l-2.758 3.152 2.758 3.15a1.2 1.2 0 0 1 0 1.698z"/>
                                </svg>
                            </span>
                        </div>
                    }
                } else { html! {} }}
                <div>
                {if let Some(lens_data) = self.lens_data.as_ref() {
                    html! {
                        <div class="flex flex-row items-center">
                            <input
                                class="border-b-4 border-neutral-600 pt-3 pb-1 bg-neutral-800 text-white text-2xl outline-none active:outline-none focus:outline-none caret-white"
                                type="text"
                                spellcheck="false"
                                tabindex="-1"
                                value={lens_data.display_name.to_string()}
                                oninput={link.callback(|_| Msg::UpdateDisplayName)}
                                ref={self._name_input_ref.clone()}
                            />
                            {if self.is_saving_name {
                                html! {
                                    <icons::RefreshIcon
                                        classes={"ml-2 text-cyan-500"}
                                        width="w-6"
                                        height="h-6"
                                        animate_spin={true}
                                    />
                                }
                            } else {
                                html! {}
                            }}
                        </div>
                    }
                } else {
                    html! {
                        <h2 class="bold text-xl ">{"Loading..."}</h2>
                    }
                }}
                </div>
                <div class="mt-4">
                    <AddSourceComponent
                        on_error={link.callback(Msg::SetError)}
                        on_update={link.callback(|_| Msg::Reload)}
                        lens_identifier={self.lens_identifier.clone()}
                    />
                </div>
                <div class="mt-8">
                    {if let Some(paginator) = self.lens_source_paginator.clone() {
                        let filter = self.source_filter;
                        html! {
                            <SourceTable
                                sources={self.lens_sources.clone().unwrap_or_default()}
                                paginator={paginator.clone()}
                                selected_filter={self.source_filter}
                                is_loading={self.is_loading_lens_sources}
                                on_delete={link.callback(Msg::DeleteLensSource)}
                                on_refresh={link.callback(move |_| Msg::ReloadSources { page: paginator.page, filter })}
                                on_select_page={link.callback(move |page| Msg::ReloadSources { page, filter })}
                                on_select_filter={link.callback(Msg::SetFilter)}
                            />
                        }
                    } else { html! {} }}
                </div>
            </div>
        }
    }
}

#[derive(Properties, PartialEq)]
struct LensSourceComponentProps {
    source: LensSource,
    on_delete: Callback<LensSource>,
}

#[function_component(LensSourceComponent)]
fn lens_source_comp(props: &LensSourceComponentProps) -> Html {
    let source = props.source.clone();
    let callback = props.on_delete.clone();
    let is_deleting = use_state_eq(|| false);
    let auth_status = use_context::<AuthStatus>().expect("Ctxt not set up");
    let ext = props
        .source
        .display_name
        .split('.')
        .last()
        .unwrap_or("")
        .to_string();
    let doc_type_icon = match source.doc_type {
        LensDocType::Audio => html! {
            <icons::FileExtIcon ext={"mp3"} class="h-4 w-4" />
        },
        LensDocType::GDrive => html! { <icons::GDrive /> },
        LensDocType::Web => html! { <icons::GlobeIcon width="w-4" height="h-4" /> },
        LensDocType::Upload => {
            html! { <icons::FileExtIcon class={classes!("w-4", "h-4")} ext={ext} /> }
        }
    };

    let status_icon = match source.status.as_ref() {
        "Deployed" => html! { <icons::BadgeCheckIcon classes="fill-green-500" /> },
        // todo: show error message in tooltip?
        "Failed" | "Unknown" => html! { <icons::Warning classes="text-yellow-500" /> },
        _ => html! { <icons::RefreshIcon animate_spin={true} /> },
    };

    let on_delete: Callback<MouseEvent> = {
        let source = source.clone();
        let is_deleting = is_deleting.clone();
        Callback::from(move |_e: MouseEvent| {
            is_deleting.set(true);
            callback.emit(source.clone());
        })
    };

    let cell_styles = classes!(
        "border-b",
        "p-2",
        "border-neutral-100",
        "dark:border-neutral-700",
        "text-neutral-500",
        "dark:text-neutral-400",
    );

    let url_link = if source.url.starts_with(DOWNLOAD_PREFIX) {
        let url = source
            .url
            .get(DOWNLOAD_PREFIX.len()..)
            .unwrap_or("")
            .to_string();
        let name = source.display_name.clone();
        let download: Callback<MouseEvent> = Callback::from(move |evt: MouseEvent| {
            evt.prevent_default();
            let client = auth_status.get_client();
            let url = url.clone();
            let name = name.clone();
            spawn_local(async move {
                let response = client.download_file(&url).await;
                match response {
                    Ok(url) => {
                        download_file(&url, &name);
                    }
                    Err(err) => {
                        log::error!("Error requesting file download {:?}", err);
                    }
                }
            });
        });

        html! {
            <a onclick={download} href="" target="_blank" class="text-cyan-500 underline">
                {source.display_name.clone()}
            </a>
        }
    } else {
        html! {
            <a href={source.url.clone()} target="_blank" class="text-cyan-500 underline">
                {source.display_name.clone()}
            </a>
        }
    };

    html! {
        <tr>
            <td class={cell_styles.clone()}>
                <div class="flex flex-row justify-center">{doc_type_icon}</div>
            </td>
            <td class={cell_styles.clone()}>
                {url_link}
                <div class="text-sm text-neutral-600">{source.url.clone()}</div>
            </td>
            <td class={cell_styles.clone()}>{status_icon}</td>
            <td class={cell_styles}>
                <Btn size={BtnSize::Xs} onclick={on_delete} _type={BtnType::Danger} disabled={*is_deleting}>
                    {if *is_deleting {
                        html! {<icons::RefreshIcon height="h-4" width="h-4" animate_spin={true} />}
                    } else {
                        html! {
                            <icons::TrashIcon height="h-4" width="h-4" />
                        }
                    }}
                </Btn>
            </td>
        </tr>
    }
}

#[derive(Properties, PartialEq)]
pub struct SourceTableProps {
    sources: Vec<LensSource>,
    paginator: LensSourcePaginator,
    selected_filter: LensSourceQueryFilter,
    is_loading: bool,
    #[prop_or_default]
    on_delete: Callback<LensSource>,
    #[prop_or_default]
    on_refresh: Callback<MouseEvent>,
    #[prop_or_default]
    on_select_page: Callback<usize>,
    #[prop_or_default]
    on_select_filter: Callback<LensSourceQueryFilter>,
}

#[function_component(SourceTable)]
pub fn source_table(props: &SourceTableProps) -> Html {
    let source_html = if props.sources.is_empty() {
        html! {
            <tr>
                <td class="text-neutral-400 text-lg pt-8 text-center" colspan="4">
                    {"Try a different filter or adding a source."}
                </td>
            </tr>
        }
    } else {
        props.sources
            .iter()
            .map(|x| html! { <LensSourceComponent on_delete={props.on_delete.clone()} source={x.clone()} /> })
            .collect::<Html>()
    };

    let header_styles = classes!(
        "border-b",
        "dark:border-neutral-600",
        "font-medium",
        "p-2",
        "text-neutral-400",
        "dark:text-neutral-200",
        "text-left"
    );

    let filters = LensSourceQueryFilter::iter()
        .map(|x| {
            let btn_type = if x == props.selected_filter {
                BtnType::Primary
            } else {
                BtnType::Default
            };

            let on_select = props.on_select_filter.clone();
            let cb = Callback::from(move |_| on_select.emit(x));
            html! {
                <Btn
                    size={BtnSize::Sm}
                    _type={btn_type}
                    onclick={cb}
                >
                    {x.to_string()}
                </Btn>
            }
        })
        .collect::<Html>();

    html! {
        <div class="flex flex-col">
            <div class="flex flex-col gap-2 md:gap-0 md:flex-row items-center justify-between border-b border-neutral-700 pb-2">
                <div class="font-bold">{format!("Data Sources ({})", props.paginator.num_items)}</div>
                <div class="flex flex-row gap-2 items-center">
                    <span class="text-sm font-semibold">{"Filter:"}</span>
                    {filters}
                </div>
                <Btn size={BtnSize::Sm} onclick={props.on_refresh.clone()}>
                    <icons::RefreshIcon
                        classes="mr-1"
                        width="w-3"
                        height="h-3"
                        animate_spin={props.is_loading}
                    />
                    {"Refresh"}
                </Btn>
            </div>
            {if props.is_loading {
                html! {
                    <div class="flex flex-row place-content-center mt-8">
                        <icons::RefreshIcon
                            width="w-8"
                            height="h-8"
                            animate_spin={props.is_loading}
                        />
                    </div>
                }
            } else {
                html! {
                    <>
                        <table class="table-auto text-sm border-collapse">
                            <thead>
                                <tr>
                                    <th class={header_styles.clone()}></th>
                                    <th class={header_styles.clone()}>{"Document"}</th>
                                    <th class={header_styles.clone()}></th>
                                    <th class={header_styles}></th>
                                </tr>
                            </thead>
                            <tbody>{source_html}</tbody>
                        </table>
                        {if props.paginator.num_pages > 1 {
                            html! {
                                <div>
                                    <Paginator
                                        disabled={props.is_loading}
                                        cur_page={props.paginator.page}
                                        num_pages={props.paginator.num_pages}
                                        on_select_page={props.on_select_page.clone()}
                                    />
                                </div>
                            }
                        } else {
                            html! {}
                        }}
                    </>
                }
            }}
        </div>
    }
}
