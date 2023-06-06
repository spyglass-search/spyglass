use crate::client::LensSource;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};

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

#[derive(Clone, Copy, Serialize, PartialEq, EnumIter, Display)]
pub enum LensSourceQueryFilter {
    #[strum(serialize = "All")]
    All,
    #[strum(serialize = "Completed")]
    Completed,
    #[strum(serialize = "In Progress")]
    InProgress,
    #[strum(serialize = "Failed")]
    Failed,
    #[strum(serialize = "Not Started")]
    NotStarted,
}

impl Default for LensSourceQueryFilter {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Serialize)]
pub struct GetLensSourceRequest {
    pub page: usize,
    pub filter: LensSourceQueryFilter,
}

#[derive(Deserialize)]
pub struct GetLensSourceResponse {
    pub page: usize,
    pub num_items: usize,
    pub num_pages: usize,
    pub results: Vec<LensSource>,
}
