use shared::event::{ClientEvent, ClientInvoke, InstallLensParams};
use shared::response::{InstallableLens, LensResult};
use std::collections::{HashMap, HashSet};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::components::lens::LensEvent;
use crate::components::{icons, lens::LibraryLens, Header};
use crate::invoke;
use crate::utils::RequestState;

async fn fetch_available_lenses() -> Option<Vec<LensResult>> {
    match invoke(ClientInvoke::ListInstallableLenses.as_ref(), JsValue::NULL).await {
        Ok(results) => match serde_wasm_bindgen::from_value::<Vec<InstallableLens>>(results) {
            Ok(lenses) => {
                let parsed: Vec<LensResult> = lenses
                    .iter()
                    .map(|lens| LensResult {
                        author: lens.author.clone(),
                        name: lens.name.clone(),
                        label: lens.label(),
                        description: lens.description.clone(),
                        hash: lens.sha.clone(),
                        html_url: Some(lens.html_url.clone()),
                        download_url: Some(lens.download_url.clone()),
                        lens_type: shared::response::LensType::Lens,
                        categories: lens.categories.clone(),
                        ..Default::default()
                    })
                    .collect();

                Some(parsed)
            }
            Err(e) => {
                log::error!("Unable to deserialize results: {}", e);
                None
            }
        },
        Err(e) => {
            log::error!("Error: {:?}", e);
            None
        }
    }
}

pub struct DiscoverPage {
    installable: Vec<LensResult>,
    installing: HashSet<String>,
    req_available: RequestState,
    // Filte lenses by some keyword
    filter_input: NodeRef,
    filter_string: Option<String>,
    // Filter lenses by category
    category_filter: Option<String>,
    category_input: NodeRef,
    category_list: Vec<(String, String)>,
}

pub enum Msg {
    FetchAvailable,
    HandleCategoryFilter,
    HandleFilter,
    HandleLensEvent(LensEvent),
    SetAvailable(Option<Vec<LensResult>>),
}

impl Component for DiscoverPage {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();
        link.send_message(Msg::FetchAvailable);

        // Handle refreshing the list when a lens is installed.
        {
            let link = link.clone();
            spawn_local(async move {
                let cb = Closure::wrap(Box::new(move |_| {
                    link.send_message(Msg::FetchAvailable);
                }) as Box<dyn Fn(JsValue)>);

                let _ = crate::listen(ClientEvent::RefreshDiscover.as_ref(), &cb).await;
                cb.forget();
            });
        }

        Self {
            req_available: RequestState::NotStarted,
            installable: Vec::new(),
            filter_string: None,
            filter_input: NodeRef::default(),
            installing: HashSet::new(),
            category_filter: None,
            category_input: NodeRef::default(),
            category_list: Vec::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::FetchAvailable => {
                if self.req_available == RequestState::InProgress {
                    return false;
                }

                self.req_available = RequestState::InProgress;
                link.send_future(async { Msg::SetAvailable(fetch_available_lenses().await) });

                false
            }
            Msg::HandleCategoryFilter => {
                if let Some(el) = self.category_input.cast::<HtmlInputElement>() {
                    let filter = el.value();
                    self.category_filter = if filter.is_empty() || filter == "ALL" {
                        None
                    } else {
                        Some(filter)
                    };
                    true
                } else {
                    false
                }
            }
            Msg::HandleFilter => {
                if let Some(el) = self.filter_input.cast::<HtmlInputElement>() {
                    let filter = el.value();
                    self.filter_string = if filter.is_empty() {
                        None
                    } else {
                        Some(filter)
                    };
                    true
                } else {
                    false
                }
            }
            Msg::HandleLensEvent(event) => {
                if let LensEvent::Install { name } = event {
                    self.installing.insert(name.clone());
                    spawn_local(async move {
                        let _ = crate::tauri_invoke::<_, ()>(
                            ClientInvoke::InstallLens,
                            &InstallLensParams { name: name.clone() },
                        )
                        .await;
                    });
                }

                true
            }
            Msg::SetAvailable(results) => {
                if let Some(results) = results {
                    self.installing.clear();
                    self.req_available = RequestState::Finished;
                    self.installable = results;

                    // Gather & sort the list of categories available.
                    let mut cat_counts: HashMap<String, u32> = HashMap::new();
                    for lens in self.installable.iter() {
                        for cat in &lens.categories {
                            let entry = cat_counts.entry(cat.to_string()).or_insert(0);
                            *entry += 1;
                        }
                    }

                    let mut categories: Vec<(String, String)> = cat_counts
                        .iter()
                        .map(|(cat, count)| (cat.to_owned(), format!("{cat} ({count})")))
                        .collect::<Vec<(_, _)>>();
                    categories.sort();
                    self.category_list = categories;

                    true
                } else {
                    self.req_available = RequestState::Error;
                    false
                }
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let contents = if self.req_available.is_done() {
            self.installable
                .iter()
                .filter_map(|data| {
                    if let Some(filter) = &self.filter_string {
                        if !data.name.to_lowercase().contains(filter)
                            && !data.description.to_lowercase().contains(filter)
                        {
                            return None;
                        }
                    }

                    if let Some(cat) = &self.category_filter {
                        if !data.categories.contains(cat) {
                            return None;
                        }
                    }

                    Some(html! { <LibraryLens
                        result={data.clone()}
                        onclick={link.callback(Msg::HandleLensEvent)}
                        in_progress={self.installing.contains(&data.name)}
                    /> })
                })
                .collect::<Html>()
        } else {
            html! {
                <div class="flex justify-center">
                    <div class="p-16">
                        <icons::RefreshIcon width="w-16" height="h-16" animate_spin={true} />
                    </div>
                </div>
            }
        };

        let header_icon = html! { <icons::GlobeIcon classes="mr-2" height="h-5" width="h-5" /> };
        html! {
            <div>
                <Header label="Discover" icon={header_icon}/>
                <div class="flex flex-col gap-2 p-4">
                    <div class="flex flex-row gap-2">
                        <select
                            class="text-black w-28"
                            ref={self.category_input.clone()}
                            onchange={link.callback(|_| Msg::HandleCategoryFilter)}
                        >
                            <option value="ALL">{"All"}</option>
                            {self.category_list.iter().map(|(value, label)| {
                                html!{ <option value={value.clone()}>{label.clone()}</option> }
                            }).collect::<Html>()}
                        </select>
                        <input type="text"
                            placeholder="filter lenses"
                            class="w-full rounded p-2 text-black text-sm"
                            onkeyup={link.callback(|_| Msg::HandleFilter)}
                            ref={self.filter_input.clone()}
                        />
                    </div>
                    {contents}
                </div>
            </div>
        }
    }
}
