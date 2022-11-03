use gloo::timers::callback::Timeout;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlElement, HtmlInputElement};
use yew::{html::Scope, prelude::*};

use shared::{
    event::{ClientEvent, ClientInvoke},
    response,
};

use crate::components::{ResultListData, SelectedLens, result::SearchResultItem};
use crate::{invoke, listen, open, resize_window, search_docs, search_lenses};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "clearTimeout")]
    fn clear_timeout(handle: i32);
}

const QUERY_DEBOUNCE_MS: u32 = 256;

#[derive(Debug)]
pub enum Msg {
    ClearQuery,
    ClearResults,
    Focus,
    KeyboardEvent(KeyboardEvent),
    HandleError(String),
    SearchDocs,
    SearchLenses,
    UpdateQuery(String),
    UpdateResults(Vec<ResultListData>),
}
pub struct SearchPage {
    lens: Vec<String>,
    search_results: Vec<ResultListData>,
    search_wrapper_ref: NodeRef,
    search_input_ref: NodeRef,
    selected_idx: usize,
    query: String,
    query_debounce: Option<i32>,
}

impl SearchPage {
    fn handle_selection(&mut self, link: &Scope<Self>) {
        // Grab the currently selected item
        if let Some(selected) = self.search_results.get(self.selected_idx) {
            if let Some(url) = selected.url.clone() {
                log::info!("open url: {}", url);
                spawn_local(async move {
                    let _ = open(url).await;
                });
            // Otherwise we're dealing w/ a lens, add to lens vec
            } else {
                // Add lens to list
                self.lens.push(selected.title.to_string());
                // Clear query string
                link.send_message(Msg::ClearQuery);
            }
        }
    }

    fn move_selection_down(&mut self) {
        let max_len = if self.search_results.is_empty() {
            0
        } else {
            self.search_results.len() - 1
        };
        self.selected_idx = (self.selected_idx + 1).min(max_len);
        self.scroll_to_result(self.selected_idx);
    }

    fn move_selection_up(&mut self) {
        self.selected_idx = self.selected_idx.max(1) - 1;
        self.scroll_to_result(self.selected_idx);
    }

    fn scroll_to_result(&self, idx: usize) {
        let document = gloo::utils::document();
        if let Some(el) = document.get_element_by_id(&format!("result-{}", idx)) {
            if let Ok(el) = el.dyn_into::<HtmlElement>() {
                el.scroll_into_view();
            }
        }
    }

    fn request_resize(&self) {
        if let Some(node) = self.search_wrapper_ref.cast::<HtmlElement>() {
            spawn_local(async move {
                resize_window(node.offset_height() as f64).await.unwrap();
            });
        }
    }
}

impl Component for SearchPage {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();

        {
            // Listen to refresh search results event
            let link = link.clone();
            spawn_local(async move {
                let cb = Closure::wrap(Box::new(move |_| {
                    link.send_message(Msg::ClearQuery);
                }) as Box<dyn Fn(JsValue)>);

                let _ = listen(ClientEvent::RefreshSearchResults.as_ref(), &cb).await;
                cb.forget();
            });
        }
        {
            // Listen to clear search events from backend
            let link = link.clone();
            spawn_local(async move {
                let cb = Closure::wrap(Box::new(move |_| {
                    link.send_message(Msg::ClearQuery);
                }) as Box<dyn Fn(JsValue)>);

                let _ = listen(ClientEvent::ClearSearch.as_ref(), &cb).await;
                cb.forget();
            });
        }
        {
            // Focus on the search box when we receive an "focus_window" event from
            // tauri
            let link = link.clone();
            spawn_local(async move {
                let cb = Closure::wrap(Box::new(move |_| {
                    link.send_message(Msg::Focus);
                }) as Box<dyn Fn(JsValue)>);
                let _ = listen(ClientEvent::FocusWindow.as_ref(), &cb).await;
                cb.forget();
            });
        }

        Self {
            lens: Vec::new(),
            search_results: Vec::new(),
            search_wrapper_ref: NodeRef::default(),
            search_input_ref: NodeRef::default(),
            selected_idx: 0,
            query: String::new(),
            query_debounce: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::ClearResults => {
                self.search_results = Vec::new();
                self.request_resize();
                true
            }
            Msg::ClearQuery => {
                self.search_results = Vec::new();
                self.query = "".to_string();
                if let Some(el) = self.search_input_ref.cast::<HtmlInputElement>() {
                    el.set_value("");
                }

                self.request_resize();
                true
            }
            Msg::Focus => {
                if let Some(el) = self.search_input_ref.cast::<HtmlElement>() {
                    let _ = el.focus();
                }
                self.request_resize();
                true
            }
            Msg::HandleError(msg) => {
                let window = window().unwrap();
                let _ = window.alert_with_message(&msg);
                false
            }
            Msg::KeyboardEvent(e) => {
                match e.type_().as_str() {
                    "keydown" => {
                        let key = e.key();
                        match key.as_str() {
                            // ArrowXX: Prevent cursor from moving around
                            // Tab: Prevent search box from losing focus
                            "ArrowUp" | "ArrowDown" | "Tab" => e.prevent_default(),
                            _ => (),
                        }

                        match key.as_str() {
                            // Search result navigation
                            "ArrowDown" => {
                                self.move_selection_down();
                                return true;
                            }
                            "ArrowUp" => {
                                self.move_selection_up();
                                return true;
                            }
                            _ => (),
                        }
                    }
                    "keyup" => {
                        let key = e.key();
                        // Stop propagation on these keys
                        match key.as_str() {
                            "ArrowDown" | "ArrowUp" | "Backspace" => e.stop_propagation(),
                            _ => {}
                        }

                        match key.as_str() {
                            "Enter" => self.handle_selection(link),
                            "Escape" => {
                                link.send_future(async move {
                                    let _ =
                                        invoke(ClientInvoke::Escape.as_ref(), JsValue::NULL).await;
                                    Msg::ClearQuery
                                });
                            }
                            "Backspace" => {
                                if self.query.is_empty() && !self.lens.is_empty() {
                                    log::info!("updating lenses");
                                    self.lens.pop();
                                }

                                let input: HtmlInputElement = e.target_unchecked_into();
                                link.send_message(Msg::UpdateQuery(input.value()));

                                if input.value().len() < crate::constants::MIN_CHARS {
                                    link.send_message(Msg::ClearResults);
                                }

                                return true;
                            }
                            // everything else
                            _ => {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                link.send_message(Msg::UpdateQuery(input.value()));
                            }
                        }
                    }
                    _ => {}
                }

                false
            }
            Msg::SearchLenses => {
                let query = self.query.trim_start_matches('/').to_string();
                link.send_future(async move {
                    match search_lenses(query).await {
                        Ok(results) => {
                            let results: Vec<response::LensResult> =
                                serde_wasm_bindgen::from_value(results).unwrap_or_default();

                            let results = results
                                .iter()
                                .map(|x| x.into())
                                .collect::<Vec<ResultListData>>();

                            Msg::UpdateResults(results)
                        }
                        Err(e) => Msg::HandleError(format!("Error: {:?}", e)),
                    }
                });
                false
            }
            Msg::SearchDocs => {
                let lenses = self.lens.clone();
                let query = self.query.clone();

                link.send_future(async move {
                    match search_docs(serde_wasm_bindgen::to_value(&lenses).unwrap(), query).await {
                        Ok(results) => {
                            let results: Vec<response::SearchResult> =
                                serde_wasm_bindgen::from_value(results).unwrap_or_default();

                            let results = results
                                .iter()
                                .map(|x| x.into())
                                .collect::<Vec<ResultListData>>();

                            Msg::UpdateResults(results)
                        }
                        Err(e) => Msg::HandleError(format!("Error: {:?}", e)),
                    }
                });

                false
            }
            Msg::UpdateResults(results) => {
                self.search_results = results;
                self.request_resize();
                true
            }
            Msg::UpdateQuery(query) => {
                self.query = query.clone();
                if let Some(timeout_id) = self.query_debounce {
                    clear_timeout(timeout_id);
                    self.query_debounce = None;
                }

                {
                    let link = link.clone();
                    let handle = Timeout::new(QUERY_DEBOUNCE_MS, move || {
                        if query.starts_with(crate::constants::LENS_SEARCH_PREFIX) {
                            link.send_message(Msg::SearchLenses);
                        } else if query.len() >= crate::constants::MIN_CHARS {
                            link.send_message(Msg::SearchDocs)
                        }
                    });

                    let id = handle.forget();
                    self.query_debounce = Some(id);
                }

                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let results = self.search_results
            .iter()
            .enumerate()
            .map(|(idx, res)| {
                let is_selected = idx == self.selected_idx;
                html! {
                    <SearchResultItem id={format!("result-{}", idx)} result={res.clone()} {is_selected} />
                }
            })
            .collect::<Html>();

        html! {
            <div ref={self.search_wrapper_ref.clone()} class="relative overflow-hidden rounded-xl border-neutral-600 border">
                <div class="flex flex-nowrap w-full">
                    <SelectedLens lens={self.lens.clone()} />
                    <input
                        ref={self.search_input_ref.clone()}
                        id="searchbox"
                        type="text"
                        class="bg-neutral-800 text-white text-5xl py-4 px-6 overflow-hidden flex-1 outline-none active:outline-none focus:outline-none"
                        placeholder="Search"
                        onkeyup={link.callback(Msg::KeyboardEvent)}
                        onkeydown={link.callback(Msg::KeyboardEvent)}
                        spellcheck="false"
                        tabindex="-1"
                    />
                </div>
                <div class="overflow-y-auto overflow-x-hidden h-full">{ results }</div>
            </div>
        }
    }
}
