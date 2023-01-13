use gloo::events::EventListener;
use gloo::timers::callback::Timeout;
use num_format::{Buffer, Locale};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlElement, HtmlInputElement};
use yew::{html::Scope, prelude::*};

use shared::{
    event::{ClientEvent, ClientInvoke},
    response::{self, SearchMeta, SearchResult, SearchResults},
};

use crate::components::{
    result::{LensResultItem, SearchResultItem},
    SelectedLens,
};
use crate::{invoke, listen, open, resize_window, search_docs, search_lenses};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "clearTimeout")]
    fn clear_timeout(handle: i32);
}

const QUERY_DEBOUNCE_MS: u32 = 256;

#[derive(Clone, PartialEq, Eq)]
pub enum ResultDisplay {
    None,
    Docs,
    Lens,
}

#[derive(Clone, Debug)]
pub enum Msg {
    Blur,
    ClearFilters,
    ClearQuery,
    ClearResults,
    Focus,
    KeyboardEvent(KeyboardEvent),
    HandleError(String),
    OpenResult(SearchResult),
    SearchDocs,
    SearchLenses,
    UpdateLensResults(Vec<response::LensResult>),
    UpdateQuery(String),
    UpdateDocsResults(SearchResults),
}
pub struct SearchPage {
    lens: Vec<String>,
    docs_results: Vec<response::SearchResult>,
    lens_results: Vec<response::LensResult>,
    result_display: ResultDisplay,
    search_meta: Option<SearchMeta>,
    search_wrapper_ref: NodeRef,
    search_input_ref: NodeRef,
    selected_idx: usize,
    query: String,
    query_debounce: Option<i32>,
    blur_timeout: Option<i32>,
}

impl SearchPage {
    fn handle_selection(&mut self, link: &Scope<Self>) {
        // Grab the currently selected item
        if !self.docs_results.is_empty() {
            if let Some(selected) = self.docs_results.get(self.selected_idx) {
                link.send_message(Msg::OpenResult(selected.to_owned()));
            }
        } else if let Some(selected) = self.lens_results.get(self.selected_idx) {
            // Add lens to list
            self.lens.push(selected.label.to_string());
            // Clear query string
            link.send_message(Msg::ClearQuery);
        }
    }

    fn open_result(&mut self, selected: &SearchResult) {
        let url = selected.url.clone();
        log::info!("open url: {}", url);
        spawn_local(async move {
            let _ = open(url).await;
        });
    }

    fn move_selection_down(&mut self) {
        let max_len = match self.result_display {
            ResultDisplay::Docs => (self.docs_results.len() - 1).max(0),
            ResultDisplay::Lens => (self.lens_results.len() - 1).max(0),
            _ => 0,
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
        if let Some(el) = document.get_element_by_id(&format!("result-{idx}")) {
            if let Ok(el) = el.dyn_into::<HtmlElement>() {
                el.scroll_into_view();
            }
        }
    }

    fn request_resize(&self) {
        if let Some(node) = self.search_wrapper_ref.cast::<HtmlElement>() {
            spawn_local(async move {
                let _ = resize_window(node.offset_height() as f64).await;
            });
        }
    }
}

impl Component for SearchPage {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();

        // Listen to onblur events so we can hide the search bar
        if let Some(wind) = window() {
            let link_clone = link.clone();
            let on_blur = EventListener::new(&wind, "blur", move |_| {
                link_clone.send_message(Msg::Blur);
            });

            let link_clone = link.clone();
            let on_focus = EventListener::new(&wind, "focus", move |_| {
                link_clone.send_message(Msg::Focus);
            });

            on_blur.forget();
            on_focus.forget();
        }

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
                    link.send_message_batch(vec![
                        Msg::ClearFilters,
                        Msg::ClearResults,
                        Msg::ClearQuery,
                    ]);
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
            docs_results: Vec::new(),
            lens_results: Vec::new(),
            result_display: ResultDisplay::None,
            search_meta: None,
            search_wrapper_ref: NodeRef::default(),
            search_input_ref: NodeRef::default(),
            selected_idx: 0,
            query: String::new(),
            query_debounce: None,
            blur_timeout: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();
        match msg {
            Msg::ClearFilters => {
                self.lens.clear();
                true
            }
            Msg::ClearResults => {
                self.selected_idx = 0;
                self.docs_results.clear();
                self.lens_results.clear();
                self.search_meta = None;
                self.result_display = ResultDisplay::None;
                self.request_resize();
                true
            }
            Msg::ClearQuery => {
                self.selected_idx = 0;
                self.docs_results.clear();
                self.lens_results.clear();
                self.search_meta = None;
                self.query = "".to_string();
                if let Some(el) = self.search_input_ref.cast::<HtmlInputElement>() {
                    el.set_value("");
                }

                self.request_resize();
                true
            }
            Msg::Blur => {
                let link = link.clone();
                // Handle the hide as a timeout since there's a brief moment when
                // alt-tabbing / clicking on the task will yield a blur event & then a
                // focus event.
                let handle = Timeout::new(100, move || {
                    spawn_local(async move {
                        let _ = invoke(ClientInvoke::Escape.as_ref(), JsValue::NULL).await;
                        link.send_message(Msg::ClearQuery);
                    });
                });

                self.blur_timeout = Some(handle.forget());
                false
            }
            Msg::Focus => {
                if let Some(el) = self.search_input_ref.cast::<HtmlInputElement>() {
                    let _ = el.focus();
                }
                self.request_resize();

                if let Some(timeout) = self.blur_timeout {
                    clear_timeout(timeout);
                    self.blur_timeout = None;
                }

                true
            }
            Msg::HandleError(msg) => {
                if let Some(window) = window() {
                    let _ = window.alert_with_message(&msg);
                } else {
                    log::error!("{}", msg);
                }
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
                            "ArrowDown" | "ArrowUp" => {}
                            "Enter" => self.handle_selection(link),
                            "Escape" => {
                                link.send_future(async move {
                                    let _ =
                                        invoke(ClientInvoke::Escape.as_ref(), JsValue::NULL).await;
                                    Msg::ClearQuery
                                });
                            }
                            "Backspace" => {
                                let input: HtmlInputElement = e.target_unchecked_into();

                                if self.query.is_empty() && !self.lens.is_empty() {
                                    log::info!("updating lenses");
                                    self.lens.pop();
                                }

                                link.send_message(Msg::UpdateQuery(input.value()));
                                if input.value().len() < crate::constants::MIN_CHARS {
                                    link.send_message(Msg::ClearResults);
                                }

                                return true;
                            }
                            "Tab" => {
                                // Tab completion for len results only
                                if self.result_display == ResultDisplay::Lens {
                                    self.handle_selection(link);
                                }
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
            Msg::OpenResult(result) => {
                self.open_result(&result);
                false
            }
            Msg::SearchLenses => {
                let query = self.query.trim_start_matches('/').to_string();
                link.send_future(async move {
                    match search_lenses(query).await {
                        Ok(results) => Msg::UpdateLensResults(
                            serde_wasm_bindgen::from_value(results).unwrap_or_default(),
                        ),
                        Err(e) => Msg::HandleError(format!("Error: {e:?}")),
                    }
                });
                false
            }
            Msg::SearchDocs => {
                let lenses = self.lens.clone();
                let query = self.query.clone();

                link.send_future(async move {
                    match serde_wasm_bindgen::to_value(&lenses) {
                        Ok(lenses) => match search_docs(lenses, query).await {
                            Ok(results) => match serde_wasm_bindgen::from_value(results) {
                                Ok(deser) => Msg::UpdateDocsResults(deser),
                                Err(e) => Msg::HandleError(format!("Error: {e:?}")),
                            },
                            Err(e) => Msg::HandleError(format!("Error: {e:?}")),
                        },
                        Err(e) => Msg::HandleError(format!("Error: {e:?}")),
                    }
                });

                false
            }
            Msg::UpdateLensResults(results) => {
                self.lens_results = results;
                self.docs_results.clear();
                self.result_display = ResultDisplay::Lens;
                self.request_resize();
                true
            }
            Msg::UpdateDocsResults(results) => {
                self.docs_results = results.results;
                self.search_meta = Some(results.meta);
                self.lens_results.clear();
                self.result_display = ResultDisplay::Docs;
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

        let results = match self.result_display {
            ResultDisplay::None => html! { },
            ResultDisplay::Docs => {
                self.docs_results
                    .iter()
                    .enumerate()
                    .map(|(idx, res)| {
                        let is_selected = idx == self.selected_idx;
                        let open_msg = Msg::OpenResult(res.to_owned());
                        html! {
                            <SearchResultItem
                                 id={format!("result-{idx}")}
                                 onclick={link.callback(move |_| open_msg.clone())}
                                 result={res.clone()}
                                 {is_selected}
                            />
                        }
                    })
                    .collect::<Html>()
            },
            ResultDisplay::Lens => {
                self.lens_results
                    .iter()
                    .enumerate()
                    .map(|(idx, res)| {
                        let is_selected = idx == self.selected_idx;
                        html! {
                            <LensResultItem id={format!("result-{idx}")} result={res.clone()} {is_selected} />
                        }
                    })
                    .collect::<Html>()
            }
        };

        let search_meta = if let Some(meta) = &self.search_meta {
            let mut num_docs = Buffer::default();
            num_docs.write_formatted(&meta.num_docs, &Locale::en);

            let mut wall_time = Buffer::default();
            wall_time.write_formatted(&meta.wall_time_ms, &Locale::en);

            html! {
                <div class="bg-neutral-900 text-neutral-500 text-xs px-4 py-2 flex flex-row items-center">
                    <div>
                        {"Searched "}
                        <span class="text-cyan-600">{num_docs}</span>
                        {" documents in "}
                        <span class="text-cyan-600">{wall_time}{" ms"}</span>
                    </div>
                    <div class="ml-auto flex flex-row align-middle items-center">
                        {"Use"}
                        <div class="border border-neutral-500 rounded bg-neutral-400 text-black p-0.5 mx-1">
                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-2 h-2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M4.5 10.5L12 3m0 0l7.5 7.5M12 3v18" />
                            </svg>
                        </div>
                        {"and"}
                        <div class="border border-neutral-500 rounded bg-neutral-400 text-black p-0.5 mx-1">
                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-2 h-2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M19.5 13.5L12 21m0 0l-7.5-7.5M12 21V3" />
                            </svg>
                        </div>
                        {"to select."}
                        <div class="border border-neutral-500 rounded bg-neutral-400 text-black px-0.5 mx-1 text-[8px]">
                            {"Enter"}
                        </div>
                        {"to open."}
                    </div>
                </div>
            }
        } else {
            html! {}
        };

        html! {
            <div ref={self.search_wrapper_ref.clone()}
                class="relative overflow-hidden rounded-xl border-neutral-600 border"
                onclick={link.callback(|_| Msg::Focus)}
            >
                <div class="flex flex-nowrap w-full">
                    <SelectedLens lens={self.lens.clone()} />
                    <input
                        ref={self.search_input_ref.clone()}
                        id="searchbox"
                        type="text"
                        class="bg-neutral-800 text-white text-5xl py-4 px-6 overflow-hidden flex-1 outline-none active:outline-none focus:outline-none caret-white"
                        placeholder="Search"
                        onkeyup={link.callback(Msg::KeyboardEvent)}
                        onkeydown={link.callback(Msg::KeyboardEvent)}
                        onclick={link.callback(|_| Msg::Focus)}
                        spellcheck="false"
                        tabindex="-1"
                    />
                </div>
                <div class="overflow-y-auto overflow-x-hidden h-full">
                    {results}
                </div>
                {search_meta}
            </div>
        }
    }
}
