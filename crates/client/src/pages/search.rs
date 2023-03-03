use gloo::timers::callback::Timeout;
use gloo::{events::EventListener, utils::window};
use num_format::{Buffer, Locale};
use shared::config::{KeyCode, ModifiersState};
use shared::config::{UserAction, UserActionDefinition, UserActionSettings};
use shared::event::CopyContext;
use shared::response::SearchResultTemplate;
use std::str::FromStr;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlElement, HtmlInputElement};
use yew::{html::Scope, prelude::*};

use shared::{
    event::{ClientEvent, ClientInvoke, OpenResultParams},
    response::{self, SearchMeta, SearchResult, SearchResults},
};

use crate::components::{
    icons,
    result::{LensResultItem, SearchResultItem},
    SelectedLens,
};
use crate::{invoke, listen, resize_window, search_docs, search_lenses, tauri_invoke};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "clearTimeout")]
    fn clear_timeout(handle: JsValue);
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
    SetCurrentActions(UserActionSettings),
    OpenResult(SearchResult),
    UserActionComplete(String),
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
    query_debounce: Option<JsValue>,
    blur_timeout: Option<JsValue>,
    is_searching: bool,
    pressed_key: Option<KeyCode>,
    executed_key: Option<KeyCode>,
    executed_action: Option<String>,
    modifier: ModifiersState,
    action_settings: Option<UserActionSettings>,
}

impl SearchPage {
    // Helper to access the currently configured user actions
    async fn fetch_user_actions() -> UserActionSettings {
        match invoke(ClientInvoke::LoadUserActions.as_ref(), JsValue::NULL).await {
            Ok(results) => match serde_wasm_bindgen::from_value(results) {
                Ok(parsed) => parsed,
                Err(e) => {
                    log::error!("Unable to deserialize results: {}", e.to_string());
                    UserActionSettings::default()
                }
            },
            Err(e) => {
                log::error!("Error fetching user settings: {:?}", e);
                UserActionSettings::default()
            }
        }
    }

    fn has_context_action(&self, settings: &UserActionSettings) -> bool {
        if !self.docs_results.is_empty() {
            if let Some(selected) = self.docs_results.get(self.selected_idx) {
                for ctx_action in &settings.context_actions {
                    if ctx_action.is_applicable(selected) {
                        return true;
                    }
                }
            }
        }

        false
    }

    // Helper used to execute the specified user action
    async fn execute_action(
        action: UserActionDefinition,
        selected: SearchResult,
        link: Scope<Self>,
    ) {
        let template_input = SearchResultTemplate::from(selected);
        match action.action {
            UserAction::OpenApplication(app_path, argument) => {
                let reg = handlebars::Handlebars::new();
                let url = match reg.render_template(argument.as_str(), &template_input) {
                    Ok(val) => val,
                    Err(_) => template_input.url.clone(),
                };
                Timeout::new(500, move || {
                    spawn_local(async move {
                        link.send_message(Msg::UserActionComplete(action.label.clone()));
                    });
                })
                .forget();
                spawn_local(async move {
                    if let Err(err) = tauri_invoke::<OpenResultParams, ()>(
                        ClientInvoke::OpenResult,
                        OpenResultParams {
                            url,
                            application: Some(app_path.clone()),
                        },
                    )
                    .await
                    {
                        let window = window();
                        let _ = window.alert_with_message(&err);
                    }
                });
            }
            UserAction::CopyToClipboard(copy_template) => {
                let reg = handlebars::Handlebars::new();
                let copy_txt = match reg.render_template(copy_template.as_str(), &template_input) {
                    Ok(val) => val,
                    Err(_) => template_input.url.clone(),
                };
                Timeout::new(500, move || {
                    spawn_local(async move {
                        link.send_message(Msg::UserActionComplete(action.label.clone()));
                    });
                })
                .forget();

                spawn_local(async move {
                    if let Err(err) = tauri_invoke::<CopyContext, ()>(
                        ClientInvoke::CopyToClipboard,
                        CopyContext { txt: copy_txt },
                    )
                    .await
                    {
                        let window = window();
                        let _ = window.alert_with_message(&err);
                    }
                });
            }
        }
    }

    fn handle_selection(&mut self, link: &Scope<Self>) {
        // Grab the currently selected item
        if !self.docs_results.is_empty() {
            if let Some(selected) = self.docs_results.get(self.selected_idx) {
                link.send_message(Msg::OpenResult(selected.to_owned()));
            }
        } else if let Some(selected) = self.lens_results.get(self.selected_idx) {
            // Add lens to list
            self.lens.push(selected.label.to_string());
            // Clear query string,
            link.send_message(Msg::ClearQuery);
        }
    }

    fn open_result(&mut self, selected: &SearchResult) {
        let url = selected.url.clone();
        log::info!("open url: {}", url);
        spawn_local(async move {
            if let Err(err) = tauri_invoke::<OpenResultParams, ()>(
                ClientInvoke::OpenResult,
                OpenResultParams {
                    url,
                    application: None,
                },
            )
            .await
            {
                let window = window();
                let _ = window.alert_with_message(&err);
            }
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

        //fire event to update available actions
    }

    fn move_selection_up(&mut self) {
        self.selected_idx = self.selected_idx.max(1) - 1;
        self.scroll_to_result(self.selected_idx);
        //fire event to update available actions
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

        // Setup user actions
        {
            link.send_future(async {
                Msg::SetCurrentActions(SearchPage::fetch_user_actions().await)
            });
        }

        // Listen to onblur events so we can hide the search bar
        {
            let wind = window();
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
            is_searching: false,
            action_settings: None,
            pressed_key: None,
            executed_key: None,
            executed_action: None,
            modifier: ModifiersState::empty(),
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
            Msg::SetCurrentActions(actions) => {
                self.action_settings = Some(actions);
                false
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

                if let Some(timeout) = &self.blur_timeout {
                    clear_timeout(timeout.clone());
                    self.blur_timeout = None;
                }

                true
            }
            Msg::HandleError(msg) => {
                let window = window();
                let _ = window.alert_with_message(&msg);
                false
            }
            Msg::UserActionComplete(_) => {
                self.executed_action = None;
                true
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

                        let mut new_key: bool = false;
                        match KeyCode::from_str(key.to_uppercase().as_str()) {
                            Ok(key_code) => match key_code {
                                KeyCode::Unidentified(_) => (),
                                _ => match self.pressed_key {
                                    Some(key) => {
                                        if !key.eq(&key_code) {
                                            new_key = true;
                                            self.pressed_key = Some(key_code);
                                        }
                                    }
                                    None => {
                                        new_key = true;
                                        self.pressed_key = Some(key_code);
                                    }
                                },
                            },
                            Err(error) => log::error!("Error processing key {:?}", error),
                        }

                        self.modifier.set(ModifiersState::ALT, e.alt_key());
                        self.modifier.set(ModifiersState::CONTROL, e.ctrl_key());
                        self.modifier.set(ModifiersState::SHIFT, e.shift_key());
                        self.modifier.set(ModifiersState::SUPER, e.meta_key());

                        if new_key {
                            if let Some(actions) = &self.action_settings {
                                if !self.docs_results.is_empty()
                                    && !self.modifier.is_empty()
                                    && self.pressed_key.is_some()
                                {
                                    let context = self.docs_results.get(self.selected_idx);
                                    if let Some(action) = actions.get_triggered_action(
                                        &self.modifier,
                                        &self.pressed_key.unwrap(),
                                        context,
                                    ) {
                                        let exec_context = context.cloned();
                                        match &action.status_msg {
                                            Some(status) => {
                                                self.executed_action = Some(status.clone());
                                            }
                                            None => {
                                                self.executed_action =
                                                    Some(format!("Executing {}", action.label));
                                            }
                                        }
                                        let link = link.clone();
                                        spawn_local(async move {
                                            SearchPage::execute_action(
                                                action,
                                                exec_context.unwrap(),
                                                link,
                                            )
                                            .await;
                                        });

                                        self.executed_key = self.pressed_key;
                                        e.prevent_default();
                                        return true;
                                    }
                                }
                            }
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

                        self.modifier.set(ModifiersState::ALT, e.alt_key());
                        self.modifier.set(ModifiersState::CONTROL, e.ctrl_key());
                        self.modifier.set(ModifiersState::SHIFT, e.shift_key());
                        self.modifier.set(ModifiersState::SUPER, e.meta_key());

                        let mut executed_key_released = false;

                        match KeyCode::from_str(key.to_uppercase().as_str()) {
                            Ok(key_code) => {
                                if self.executed_key.is_some()
                                    && self.executed_key.unwrap().eq(&key_code)
                                {
                                    executed_key_released = true;
                                    self.executed_key = None;
                                }
                                if let Some(key) = self.pressed_key {
                                    if key.eq(&key_code) {
                                        self.pressed_key = None;
                                    }
                                }

                                match key_code {
                                    KeyCode::ArrowUp
                                    | KeyCode::ArrowDown
                                    | KeyCode::CapsLock
                                    | KeyCode::Unidentified(_)
                                    | KeyCode::ControlLeft
                                    | KeyCode::ControlRight
                                    | KeyCode::AltLeft
                                    | KeyCode::AltRight
                                    | KeyCode::ShiftLeft
                                    | KeyCode::ShiftRight
                                    | KeyCode::Abort
                                    | KeyCode::End
                                    | KeyCode::Home
                                    | KeyCode::PageDown
                                    | KeyCode::PageUp
                                    | KeyCode::AudioVolumeDown
                                    | KeyCode::AudioVolumeMute
                                    | KeyCode::AudioVolumeUp
                                    | KeyCode::MediaPlayPause
                                    | KeyCode::MediaSelect
                                    | KeyCode::MediaStop
                                    | KeyCode::MediaTrackNext
                                    | KeyCode::MediaTrackPrevious => {}
                                    KeyCode::Enter => {
                                        if !executed_key_released {
                                            self.handle_selection(link)
                                        }
                                    }
                                    KeyCode::Escape => {
                                        link.send_future(async move {
                                            let _ = invoke(
                                                ClientInvoke::Escape.as_ref(),
                                                JsValue::NULL,
                                            )
                                            .await;
                                            Msg::ClearQuery
                                        });
                                    }
                                    KeyCode::Backspace => {
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
                                    KeyCode::Tab => {
                                        // Tab completion for len results only
                                        if self.result_display == ResultDisplay::Lens {
                                            self.handle_selection(link);
                                        }
                                    }
                                    // everything else
                                    _ => {
                                        if !executed_key_released {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            link.send_message(Msg::UpdateQuery(input.value()));
                                        }
                                    }
                                }
                            }
                            Err(error) => log::error!("Error processing key {:?}", error),
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
                self.is_searching = true;
                link.send_future(async move {
                    match search_lenses(query).await {
                        Ok(results) => {
                            let lens_results = {
                                match serde_wasm_bindgen::from_value(results) {
                                    Ok(results) => results,
                                    Err(err) => {
                                        log::error!(
                                            "Unable to deserialize search_lenses result: {:?}",
                                            err
                                        );
                                        Vec::new()
                                    }
                                }
                            };

                            Msg::UpdateLensResults(lens_results)
                        }
                        Err(e) => Msg::HandleError(format!("Error: {e:?}")),
                    }
                });
                false
            }
            Msg::SearchDocs => {
                let lenses = self.lens.clone();
                let query = self.query.clone();
                self.is_searching = true;
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
                self.is_searching = false;
                true
            }
            Msg::UpdateDocsResults(results) => {
                if self.query == results.meta.query {
                    self.docs_results = results.results;
                    self.search_meta = Some(results.meta);
                    self.lens_results.clear();
                    self.result_display = ResultDisplay::Docs;
                    self.request_resize();
                    self.is_searching = false;
                }
                true
            }
            Msg::UpdateQuery(query) => {
                self.query = query.clone();
                if let Some(timeout_id) = &self.query_debounce {
                    clear_timeout(timeout_id.clone());
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

            let running_action = if let Some(action) = &self.executed_action {
                html! {
                    <div class="flex flex-row gap-1 items-center">
                        <icons::RefreshIcon width="w-3" height="h-3" animate_spin={true} />
                        <span class="text-cyan-600">{action}</span>
                    </div>
                }
            } else {
                html! {
                    <div>
                        {"Searched "}
                        <span class="text-cyan-600">{num_docs}</span>
                        {" documents in "}
                        <span class="text-cyan-600">{wall_time}{" ms."}</span>
                    </div>
                }
            };

            html! {
                <div class="flex flex-row justify-between w-full items-center align-middle">
                    {running_action}
                    <div class="flex flex-row align-middle items-center">
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
            let is_searching_indicator = if self.is_searching {
                html! {
                    <div class="flex flex-row gap-1 items-center">
                        <icons::RefreshIcon width="w-3" height="h-3" animate_spin={true} />
                        {"Searching..."}
                    </div>
                }
            } else {
                html! {}
            };

            html! {
                <>
                    {is_searching_indicator}
                    <div class="ml-auto flex flex-row items-center align-middle">
                    {"Use"}
                    <div class="mx-1 rounded border border-neutral-500 bg-neutral-400 px-1 text-black text-[8px]">
                        {"/"}
                    </div>
                    {"to select a lens."}
                    <div class="mx-1 rounded border border-neutral-500 bg-neutral-400 px-0.5 text-[8px] text-black">
                        {"Type"}
                    </div>
                    {"to search."}
                    </div>
                </>
            }
        };

        let noop = html! {};

        let _custom_actions = if let Some(settings) = &self.action_settings {
            if !self.docs_results.is_empty()
                && (!settings.actions.is_empty() || self.has_context_action(settings))
            {
                html! {
                    <div class="border-l border-neutral-500 px-0 py-1.5 hover:bg-stone-700 hover:border-stone-500 flex-none w-6">
                      <div class="border border-neutral-500 rounded bg-neutral-400 text-black p-0.5 mx-1">
                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-2 h-2">
                          <path stroke-linecap="round" stroke-linejoin="round" d="M13.5 6H5.25A2.25 2.25 0 003 8.25v10.5A2.25 2.25 0 005.25 21h10.5A2.25 2.25 0 0018 18.75V10.5m-10.5 6L21 3m0 0h-5.25M21 3v5.25" />
                        </svg>
                      </div>
                    </div>
                }
            } else {
                noop
            }
        } else {
            noop
        };
        html! {
            <div ref={self.search_wrapper_ref.clone()}
                class="relative overflow-hidden rounded-xl border-neutral-600 border"
                onclick={link.callback(|_| Msg::Focus)}
            >
                <div class="flex flex-nowrap w-full bg-neutral-800">
                    <SelectedLens lens={self.lens.clone()} />
                    <input
                        ref={self.search_input_ref.clone()}
                        id="searchbox"
                        type="text"
                        class="bg-neutral-800 text-white text-5xl py-3 overflow-hidden flex-1 outline-none active:outline-none focus:outline-none caret-white"
                        placeholder="Search"
                        onkeyup={link.callback(Msg::KeyboardEvent)}
                        onkeydown={link.callback(Msg::KeyboardEvent)}
                        onclick={link.callback(|_| Msg::Focus)}
                        spellcheck="false"
                        tabindex="-1"
                    />
                </div>
                <div class="overflow-y-auto overflow-x-hidden h-full max-h-[640px]">
                    {results}
                </div>
                <div  class="flex flex-row w-full items-center bg-neutral-900">
                  <div class="bg-neutral-900 grow text-neutral-500 text-xs px-3 py-1.5 flex flex-row items-center gap-2">
                      {search_meta}
                  </div>
                </div>
            </div>
        }
    }
}
