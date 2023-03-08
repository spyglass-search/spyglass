use crate::components::icons;
use crate::components::icons::{ArrowTopRightOnSquare, BookOpen, ClipboardDocumentIcon};
use crate::utils::{self, OsName};
use shared::accelerator;
use shared::{
    config::{self, UserAction, UserActionDefinition},
    keyboard::ModifiersState,
};
use yew::function_component;
use yew::prelude::*;

// Label used for the default action
pub const DEFAULT_ACTION_LABEL: &str = "Open with default app";
pub const USER_ACTION_PREFIX: &str = "user-action-";

/// Properties for the action list component
#[derive(Properties, PartialEq)]
pub struct ActionsListProps {
    // The list of actions to show
    pub actions: Vec<UserActionDefinition>,
    // The currently selected action
    pub selected_action: usize,
    // The callback to call when an action is selected
    #[prop_or_default]
    pub onclick: Callback<UserActionDefinition>,
}

#[derive(Properties, PartialEq)]
pub struct UserActionProps {
    pub action: UserActionDefinition,
    pub is_selected: bool,
    pub action_id: String,
    #[prop_or_default]
    pub onclick: Callback<UserActionDefinition>,
}

#[derive(Properties, PartialEq)]
pub struct ModifierProps {
    pub modifier: ModifiersState,
}

#[function_component(ModifierIcon)]
fn modifier_icon(props: &ModifierProps) -> Html {
    let mut nodes: Vec<Html> = Vec::new();

    if props.modifier.control_key() {
        nodes.push(html! { <TextBubble>{"Ctrl"}</TextBubble> });
    }

    if props.modifier.super_key() {
        match utils::get_os() {
            OsName::MacOS => nodes.push(
                html! { <TextBubble><icons::CmdIcon height="h-3" width="w-3" /></TextBubble> },
            ),
            _ => nodes.push(
                html! { <TextBubble><icons::WinKeyIcon height="h-3" width="w-3" /></TextBubble> },
            ),
        }
    }

    if props.modifier.alt_key() {
        nodes.push(html! { <TextBubble>{"Alt"}</TextBubble> });
    }

    if props.modifier.shift_key() {
        nodes.push(html! { <TextBubble>{"Shift"}</TextBubble> });
    }

    html! { <>{nodes}</> }
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
        "hover:bg-cyan-900",
        if props.is_selected {
            "bg-cyan-900"
        } else {
            "bg-neutral-700"
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
                <TextBubble>{key_binding}</TextBubble>
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
        UserAction::OpenApplication(_, _) => {
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

#[function_component(ActionsList)]
pub fn user_actions_list(props: &ActionsListProps) -> Html {
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
        <div class="absolute bottom-8 h-32 max-h-screen w-1/2 right-0 z-20 flex flex-col overflow-hidden rounded-tl-lg bg-neutral-700 border-t-2 border-l-2 border-neutral-900">
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

#[derive(Properties, PartialEq)]
pub struct TextBubbleProps {
    pub children: Children,
}

#[function_component(TextBubble)]
pub fn txt_bubble(props: &TextBubbleProps) -> Html {
    html! {
      <div class="border border-neutral-500 rounded bg-neutral-400 text-black px-0.5 text-[8px]">
        {props.children.clone()}
      </div>
    }
}
