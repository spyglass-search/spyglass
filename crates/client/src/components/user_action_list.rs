use crate::components::icons;
use crate::components::{KeyComponent, ModifierIcon};
use crate::{tauri_invoke, utils};
use gloo::utils::window;
use handlebars::{
    Context, Handlebars, Helper, HelperResult, Output, PathAndJson, RenderContext, RenderError,
};
use shared::accelerator;
use shared::config::{self, UserAction, UserActionDefinition};
use shared::event::{ClientInvoke, CopyContext, OpenResultParams, SendToAskClippyPayload};
use shared::response::{SearchResult, SearchResultTemplate};
use yew::function_component;
use yew::platform::spawn_local;
use yew::prelude::*;

// Label used for the default action
pub const DEFAULT_ACTION_LABEL: &str = "Open with default app";
pub const USER_ACTION_PREFIX: &str = "user-action-";

#[derive(Properties, PartialEq)]
pub struct UserActionProps {
    pub action: UserActionDefinition,
    pub is_selected: bool,
    pub action_id: String,
    #[prop_or_default]
    pub onclick: Callback<UserActionDefinition>,
}

#[function_component(UserActionComponent)]
fn user_action(props: &UserActionProps) -> Html {
    let component_styles = classes!(
        "flex",
        "flex-col",
        "py-2",
        "text-sm",
        "text-white",
        "cursor-pointer",
        "active:bg-cyan-900",
        "rounded",
        if props.is_selected {
            "bg-cyan-900"
        } else {
            "bg-stone-800"
        }
    );
    let txt = props.action.label.clone();

    if let Ok(accelerator) = accelerator::parse_accelerator(
        props.action.key_binding.as_str(),
        utils::get_os().to_string().as_str(),
    ) {
        let key_binding = accelerator.key.to_str().to_string();
        let click_action = props.onclick.clone();
        let action = props.action.clone();

        let user_action = action.action.clone();
        html! {
            <div id={props.action_id.clone()} class={component_styles} onclick={Callback::from(move |_| click_action.emit(action.clone()))}>
              <div class="flex flex-row px-2 items-center gap-1">
                <ActionIcon actiontype={user_action.clone()}></ActionIcon>
                <span class="grow">{txt}</span>
                <ModifierIcon modifier={accelerator.mods}></ModifierIcon>
                <KeyComponent>{key_binding}</KeyComponent>
              </div>
            </div>
        }
    } else {
        html! {
            <div class={component_styles}>
              <div class="flex flex-row px-2">
                <icons::BookOpen/>
                <span class="grow">{txt}</span>
              </div>
            </div>
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct ActionIconProps {
    pub actiontype: UserAction,
}

#[function_component(ActionIcon)]
pub fn action_icon(props: &ActionIconProps) -> Html {
    match props.actiontype {
        UserAction::AskClippy(_) => {
            html! { <icons::ChatBubbleIcon height="h-4" width="w-4"/> }
        }
        UserAction::OpenApplication(_, _) | UserAction::OpenUrl(_) => {
            html! { <icons::ArrowTopRightOnSquare height="h-4" width="w-4"/> }
        }
        UserAction::CopyToClipboard(_) => {
            html! { <icons::ClipboardDocumentIcon height="h-4" width="w-4"/> }
        }
    }
}

/// Properties for the action list component
#[derive(Properties, PartialEq)]
pub struct ActionsListProps {
    // Should we show the action list?
    pub show: bool,
    // The list of actions to show
    pub actions: Vec<UserActionDefinition>,
    // The currently selected action
    pub selected_action: usize,
    // The callback to call when an action is selected
    #[prop_or_default]
    pub onclick: Callback<UserActionDefinition>,
}

#[function_component(ActionsList)]
pub fn user_actions_list(props: &ActionsListProps) -> Html {
    if !props.show {
        return html! {};
    }

    let mut index: usize = 1;
    let html = props.actions.iter().map(|act| {
            let is_selected = index == props.selected_action;
            let id_str = format!("{USER_ACTION_PREFIX}{index}");
            index += 1;
            html! {
                <UserActionComponent action_id={id_str} action={act.clone()} is_selected={is_selected} onclick={props.onclick.clone()} />
            }
        }).collect::<Html>();

    let default_action = UserActionDefinition {
        action: config::UserAction::OpenApplication(String::from("default"), String::from("")),
        key_binding: String::from("Enter"),
        label: String::from(DEFAULT_ACTION_LABEL),
        status_msg: Some(String::from("OpenDefaultApplication")),
    };

    let action_id = format!("{USER_ACTION_PREFIX}-0");
    html! {
        <div class="absolute bottom-8 h-32 max-h-screen w-1/2 right-0 z-20 flex flex-col overflow-hidden rounded-tl-lg bg-stone-800 border-t-2 border-l-2 border-neutral-900 p-1">
          <div class="overflow-y-auto">
            <UserActionComponent
              action_id={action_id}
              action={default_action}
              is_selected={props.selected_action == 0}
              onclick={props.onclick.clone()}
            />
            {html}
          </div>
        </div>
    }
}

/// Helper used to execute the specified user action
pub async fn execute_action(selected: SearchResult, action: UserActionDefinition) {
    let template_input = SearchResultTemplate::from(selected);
    let mut reg = handlebars::Handlebars::new();
    reg.register_helper("slice_path", Box::new(slice_path));
    reg.register_escape_fn(handlebars::no_escape);

    match action.action {
        UserAction::OpenApplication(app_path, argument) => {
            let url = match reg.render_template(argument.as_str(), &template_input) {
                Ok(val) => val,
                Err(_) => template_input.url.clone(),
            };

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
        UserAction::OpenUrl(url) => {
            let url = match reg.render_template(url.as_str(), &template_input) {
                Ok(val) => val,
                Err(_) => template_input.url.clone(),
            };

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
        UserAction::CopyToClipboard(copy_template) => {
            let copy_txt = match reg.render_template(copy_template.as_str(), &template_input) {
                Ok(val) => val,
                Err(_) => template_input.url.clone(),
            };

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

#[derive(Properties, PartialEq)]
pub struct ActionListBtnProps {
    pub show: bool,
    pub is_active: bool,
    pub onclick: Callback<MouseEvent>,
}

#[function_component(ActionListBtn)]
pub fn action_button(props: &ActionListBtnProps) -> Html {
    let classes = classes!(
        "flex",
        "flex-row",
        "items-center",
        "border-l",
        "text-sm",
        "text-neutral-500",
        "border-neutral-700",
        "px-3",
        "ml-3",
        "h-8",
        if props.is_active {
            "bg-stone-800"
        } else {
            "bg-neutral-900"
        },
        "hover:bg-stone-800",
    );

    html! {
        <button class={classes} onclick={props.onclick.clone()}>
          <KeyComponent>{"ENTER"}</KeyComponent>
          <span class="ml-1">{"to open."}</span>
        </button>
    }
}

// Helper used to take slices of a path
fn slice_path(
    helper: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let path = helper.param(0);
    let start = helper.param(1);
    let end = helper.param(2);
    let count = helper.hash_get("count");
    let full_uri = helper.hash_get("full_uri");

    log::debug!(
        "Path: {path:?} Start: {start:?} End: {end:?} Count: {count:?} Full URI: {full_uri:?}"
    );
    if let (Some(path), Some(start)) = (path, start) {
        let url = url::Url::parse(path.render().as_str());
        match url {
            Ok(mut url) => {
                let start_val = start.value();
                if let Some(start_i64) = start_val.as_i64() {
                    if let Some(segments) = url.path_segments().map(|c| c.collect::<Vec<_>>()) {
                        let start = get_start(segments.len(), start_i64);
                        let start_usize = start as usize;
                        let end = get_end(segments.len(), start, end, count) as usize;
                        log::debug!("Start: {start:?} End: {end:?} Segments {segments:?}");
                        if let Some(segment) = segments.get(start_usize..end) {
                            match full_uri.map(|uri| uri.value().as_bool().unwrap_or(false)) {
                                Some(true) => {
                                    url.set_path(segment.join("/").as_str());
                                    out.write(url.as_str())?;
                                }
                                _ => {
                                    out.write(segment.join("/").as_str())?;
                                }
                            }
                        }
                    }
                }
            }
            Err(err) => {
                log::error!("Invalid url {:?}", err);
                return Err(RenderError::new("Path is an invalid url"));
            }
        }
    } else {
        return Err(RenderError::new("A path and start are required"));
    }
    Ok(())
}

// Helper method used to calculate the start of a range based on the size of the array and the
// start index. Note the index can be negative
fn get_start(size: usize, start: i64) -> u64 {
    if start < 0 {
        if let Some(added) = size.checked_add_signed(start as isize) {
            return added.max(0).min(size) as u64;
        }
    }
    start as u64
}

// Helper method used to get the end of a sequence from the start, size, end and count. The end and
// count are both optional.
fn get_end(size: usize, start: u64, end: Option<&PathAndJson>, count: Option<&PathAndJson>) -> u64 {
    let size = size as i64;
    let max_size = size as u64;
    if let Some(end) = end {
        let value = end.value();
        if value.is_i64() {
            if let Some(val_i64) = value.as_i64() {
                if val_i64 < 0 {
                    let end_size = (size + val_i64).max(0) as u64;
                    return end_size.min(max_size);
                } else {
                    return val_i64 as u64;
                }
            }
        } else if value.is_u64() {
            return value.as_u64().unwrap();
        }
    }

    if let Some(count) = count {
        let count_val = count.value();
        if let Some(count_u64) = count_val.as_u64() {
            return start
                .checked_add(count_u64)
                .unwrap_or(max_size)
                .min(max_size);
        }
    }
    max_size
}
