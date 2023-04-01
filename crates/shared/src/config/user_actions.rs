use diff::Diff;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::accelerator;
use crate::keyboard::{KeyCode, ModifiersState};
use crate::response::SearchResult;

use super::{Tag, UserAction};

// Defines context specific actions. A context specific action
// is a list of actions that are only valid when the document selected
// matches the defined context.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Diff)]
pub struct ContextActions {
    // Defines what context must be matched for the actions to be valid
    pub context: ContextFilter,
    // The list of actions for this context
    pub actions: Vec<UserActionDefinition>,
}

impl ContextActions {
    // Helper method used to identify if the context contains an actions that should
    // be triggered by the passed in keyboard combination. Note this method does not
    // check to see if the context is valid, just if the key combination matches.
    pub fn contains_trigger(&self, modifiers: &ModifiersState, key: &KeyCode, os: &str) -> bool {
        for action in &self.actions {
            if action.is_triggered(modifiers, key, os) {
                return true;
            }
        }
        false
    }

    // Helper method used to access the action that should be triggered by the passed
    // in key combination. Note this method does not check to see if the context is valid.
    pub fn get_triggered_action(
        &self,
        modifiers: &ModifiersState,
        key: &KeyCode,
        os: &str,
    ) -> Option<UserActionDefinition> {
        for action in &self.actions {
            if action.is_triggered(modifiers, key, os) {
                return Some(action.clone());
            }
        }
        None
    }

    // Helper method used to identify if the context actions are valid based on the
    // passed in search result
    pub fn is_applicable(&self, context: &SearchResult) -> bool {
        let mut current_tags = HashMap::new();
        for (tag, value) in &context.tags {
            current_tags
                .entry(tag.clone())
                .or_insert(Vec::new())
                .push(value.clone());
        }
        // Process exclude tag first to remove unwanted items
        if let Some(exclude_types) = &self.context.exclude_tag_type {
            for tag_type in exclude_types {
                if current_tags.contains_key(tag_type.as_str()) {
                    return false;
                }
            }
        }

        if let Some(exclude_tags) = &self.context.exclude_tag {
            for (tag, value) in exclude_tags {
                if let Some(current_vals) = current_tags.get(tag.as_str()) {
                    if current_vals.contains(value) {
                        return false;
                    }
                }
            }
        }

        let tags_configured = self.context.has_tag_type.is_some() | self.context.has_tag.is_some();
        let mut include: bool = !tags_configured;
        if let Some(tags) = &self.context.has_tag_type {
            for tag in tags {
                if current_tags.contains_key(tag.as_str()) {
                    // The current context has a tag type we are
                    // looking for, set to true and break out of
                    // the loop
                    include |= true;
                    break;
                }
            }
        }

        if !include {
            if let Some(tags) = &self.context.has_tag {
                for (tag, value) in tags {
                    if let Some(current_vals) = current_tags.get(tag.as_str()) {
                        if current_vals.contains(value) {
                            // The current context has a tag we are
                            // looking for, set to true and break out of
                            // the loop
                            include |= true;
                            break;
                        }
                    }
                }
            }
        }

        if let Some(urls) = &self.context.url_like {
            let mut has_url = false;
            for url in urls {
                if url.eq(&context.url) {
                    has_url = true;
                    break;
                }
            }
            // We have urls to check so the context must match
            // one of the urls and any tags that were
            // previously defined
            include &= has_url;
        }
        include
    }
}

// Filter definition used to define what documents should match
// against the context.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Diff)]
pub struct ContextFilter {
    // Includes documents that match any of the defined tags
    pub has_tag: Option<Vec<Tag>>,
    // Includes documents that match any of the defined tag types
    pub has_tag_type: Option<Vec<String>>,
    // Excludes documents that match the specified tag
    pub exclude_tag: Option<Vec<Tag>>,
    // Exclude documents that match the defined tag type
    pub exclude_tag_type: Option<Vec<String>>,
    // Include only documents that match the specified url. When
    // set a document must match the url and any specified tags
    // to be included
    pub url_like: Option<Vec<String>>,
}

// The definition for an action
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Diff)]
pub struct UserActionDefinition {
    pub label: String,
    pub status_msg: Option<String>,
    pub action: UserAction,
    pub key_binding: String,
}

impl UserActionDefinition {
    // Helper used to identify if the key binding specified in the action definition
    // matches the passed in key code and modifiers
    pub fn is_triggered(&self, modifiers: &ModifiersState, key: &KeyCode, os: &str) -> bool {
        //todo preprocess accelerator like a normal person
        if let Ok(accelerator) = accelerator::parse_accelerator(&self.key_binding, os) {
            return accelerator.matches(modifiers, key);
        }
        false
    }
}

// The user action settings configuration provides the ability
// for the user to define custom behavior for a document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Diff)]
pub struct UserActionSettings {
    pub actions: Vec<UserActionDefinition>,
    pub context_actions: Vec<ContextActions>,
}

impl UserActionSettings {
    // Helper used to identify if the user action settings contains
    // an action that would be triggered by the passed in keyboard
    // combination.
    pub fn contains_trigger(
        &self,
        modifiers: &ModifiersState,
        key: &KeyCode,
        context: Option<&SearchResult>,
        os: &str,
    ) -> bool {
        for action in &self.actions {
            if action.is_triggered(modifiers, key, os) {
                return true;
            }
        }

        if let Some(context) = context {
            for action in &self.context_actions {
                if action.is_applicable(context) && action.contains_trigger(modifiers, key, os) {
                    return true;
                }
            }
        }
        false
    }

    // Helper method used to access the action definition for the action
    // that should be triggered by the passed in keyboard combination.
    // Note that context specific actions will be checked before general
    // actions. This allows a user to configure a general action for all
    // documents and custom actions for specific documents
    pub fn get_triggered_action(
        &self,
        modifiers: &ModifiersState,
        key: &KeyCode,
        os: &str,
        context: Option<&SearchResult>,
    ) -> Option<UserActionDefinition> {
        if let Some(context) = context {
            for action in &self.context_actions {
                if action.is_applicable(context) {
                    if let Some(context_action) = action.get_triggered_action(modifiers, key, os) {
                        return Some(context_action);
                    }
                }
            }
        }

        for action in &self.actions {
            if action.is_triggered(modifiers, key, os) {
                return Some(action.clone());
            }
        }

        None
    }
}

impl Default for UserActionSettings {
    // List of default actions when no other actions are configured
    fn default() -> Self {
        Self {
            actions: vec![
                UserActionDefinition {
                    action: UserAction::CopyToClipboard(String::from("{{ open_url }}")),
                    key_binding: String::from("CmdOrCtrl+C"),
                    label: String::from("Copy URL to Clipboard"),
                    status_msg: Some(String::from("Copying...")),
                },
                UserActionDefinition {
                    action: UserAction::AskClippy("{{ doc_id }}".into()),
                    key_binding: String::from("CmdOrCtrl+Enter"),
                    label: String::from("Ask Clippy"),
                    status_msg: None,
                },
            ],
            context_actions: vec![ContextActions {
                context: ContextFilter {
                    has_tag: Some(vec![("type".into(), "file".into())]),
                    ..Default::default()
                },
                actions: vec![UserActionDefinition {
                    label: "Open parent folder".into(),
                    status_msg: None,
                    action: UserAction::OpenUrl("{{url_parent}}".into()),
                    key_binding: "Shift+Enter".into(),
                }],
            }],
        }
    }
}
