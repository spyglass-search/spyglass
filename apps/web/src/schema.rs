use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbedConfiguration {
    pub prompt_style: EmbeddedPromptStyle,
    pub theme: Theme,
    pub header_color: Option<String>,
    pub bot_bubble_color: Option<String>,
    pub user_bubble_color: Option<String>,
    pub header_title: Option<String>,
    pub initial_chat: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EmbeddedPromptStyle {
    Research,
    Chat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Theme {
    DarkMode,
    LightMode,
}
