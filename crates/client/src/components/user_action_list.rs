use crate::components::icons::{ArrowTopRightOnSquare, BookOpen, ClipboardDocumentIcon};
use crate::components::{KeyComponent, ModifierIcon};
use crate::{tauri_invoke, utils};
use gloo::utils::window;
use shared::accelerator;
use shared::config::{self, UserAction, UserActionDefinition};
use shared::event::{ClientInvoke, CopyContext, OpenResultParams};
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
                <BookOpen/>
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
        UserAction::OpenUrl(_) | UserAction::OpenApplication(_, _) => {
            html! {
              <ArrowTopRightOnSquare height="h-4" width="w-4"/>
            }
        }
        UserAction::CopyToClipboard(_) => {
            html! {
              <ClipboardDocumentIcon height="h-4" width="w-4"/>
            }
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
