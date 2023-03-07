use crate::components::icons;
use crate::components::icons::{
    ArrowTopRightOnSquare, BookOpen, ClipboardDocumentIcon, DownArrowInBubble, UpArrowInBubble,
};
use crate::utils::{self, OsName};
use shared::accelerator;
use shared::{
    config::{self, UserAction, UserActionDefinition},
    keyboard::ModifiersState,
};
use yew::function_component;
use yew::prelude::*;

// Label used for the default action
pub const DEFAULT_ACTION_LABEL: &str = "Open with default application...";
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
    let component_styles = classes!(
        "flex",
        "justify-center",
        "items-center",
        "ml-1",
        "min-w-[24px]",
        "rounded",
        "border",
        "border-neutral-500",
        "bg-neutral-400",
        "px-0.5",
        "text-[10px]",
        "text-black"
    );

    let ctrl_icon = if props.modifier.control_key() {
        html! {
            <div class={component_styles.clone()}>
              <span>{"Ctrl"}</span>
            </div>
        }
    } else {
        html! {}
    };

    let meta_icon = if props.modifier.super_key() {
        match utils::get_os() {
            OsName::MacOS => {
                html! {
                  <div class={component_styles.clone()}>
                    <icons::CmdIcon height="h-4" width="w-4" />
                  </div>
                }
            }
            _ => {
                html! {
                  <div class={component_styles.clone()}>
                    <icons::WinKeyIcon height="h-4" width="w-4" />
                  </div>
                }
            }
        }
    } else {
        html! {}
    };

    let alt_icon = if props.modifier.alt_key() {
        html! {
            <div class={component_styles.clone()}>
               <span>{"Alt"}</span>
            </div>
        }
    } else {
        html! {}
    };

    let shift_icon = if props.modifier.shift_key() {
        html! {
            <div class={component_styles}>
              <span>{"Shift"}</span>
            </div>
        }
    } else {
        html! {}
    };

    html! {
        <>
          {ctrl_icon}
          {meta_icon}
          {alt_icon}
          {shift_icon}
        </>
    }
}

#[function_component(UserActionComponent)]
fn user_action(props: &UserActionProps) -> Html {
    let component_styles = classes!(
        "flex",
        "flex-col",
        "border-t",
        "border-neutral-600",
        "px-8",
        "py-4",
        "text-white",
        "cursor-pointer",
        "hover:bg-cyan-900",
        if props.is_selected {
            "bg-cyan-900"
        } else {
            "bg-neutral-800"
        }
    );
    let txt = props.action.label.clone();

    if let Ok(accelerator) = accelerator::parse_accelerator(
        props.action.key_binding.as_str(),
        utils::get_os().to_string().as_str(),
    ) {
        let key_binding = accelerator.key.to_str();
        let click_action = props.onclick.clone();
        let action = props.action.clone();

        let user_action = action.action.clone();
        html! {
            <div id={props.action_id.clone()} class={component_styles} onclick={Callback::from(move |_| click_action.emit(action.clone()))}>
              <div class="flex flex-row justify-center px-2">
                <div class="flex justify-center items-center">
                  <ActionIcon actiontype={user_action.clone()}></ActionIcon>
                </div>
                <span class="grow pl-2">
                  {txt}
                </span>
                <div class="flex flex-row pl-1 align-middle">
                  <ModifierIcon modifier={accelerator.mods}></ModifierIcon>
                  <div class="flex justify-center items-center ml-1 min-w-[24px] rounded border border-neutral-500 bg-neutral-400 px-0.5 text-[10px] text-black">
                    <span>{key_binding}</span>
                  </div>
                </div>
              </div>
            </div>
        }
    } else {
        html! {
            <div class={component_styles}>
              <div class="flex flex-row px-2">
                <BookOpen/>
                <span class="grow">
                  {txt}
                </span>
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
                <UserActionComponent action_id={id_str} action={act.clone()} is_selected={is_selected} onclick={props.onclick.clone()}></UserActionComponent>
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
        <div class="absolute bottom-0 right-0 bg-neutral-800 z-20 flex flex-col rounded-xl border-neutral-600 border content-start px-0 py-0 h-1/2 w-3/4 overflow-hidden">
          <div class="grow overflow-y-auto">
            <UserActionComponent action_id={action_id} action={default_action} is_selected={props.selected_action == 0} onclick={props.onclick.clone()}></UserActionComponent>
            {html}
          </div>
          <div  class="flex flex-row w-full items-center bg-neutral-900">
            <div class="bg-neutral-900 grow text-neutral-500 text-xs px-3 py-1.5 flex flex-row justify-end items-center gap-2">
              <div class="flex flex-row align-middle items-center">
                {"Use"}
                <UpArrowInBubble height="h-2" width="w-2"></UpArrowInBubble>
                {"and"}
                <DownArrowInBubble height="h-2" width="w-2"></DownArrowInBubble>
                {"to select."}
                <TextBubble txt="Enter"></TextBubble>
                {"to execute action."}
                <TextBubble txt="Esc"></TextBubble>
                {"to close window."}
              </div>
            </div>
          </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct TextBubbleProps {
    pub txt: String,
}

#[function_component(TextBubble)]
pub fn txt_bubble(props: &TextBubbleProps) -> Html {
    html! {
      <div class="mx-1 rounded border border-neutral-500 bg-neutral-400 px-0.5 text-[8px] text-black">
        {props.txt.clone()}
      </div>
    }
}
