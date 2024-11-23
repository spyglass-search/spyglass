use serde::{Deserialize, Serialize};
use ts_rs::TS;

// Generation is roughly the order things happen.
#[derive(Deserialize, Serialize, PartialEq, Eq, TS)]
#[serde(tag = "type", content = "content")]
#[ts(export)]
pub enum ChatStream {
    LoadingPrompt,
    ChatStart,
    Token(String),
    ChatDone,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub enum ChatRole {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct LlmSession {
    pub messages: Vec<ChatMessage>,
}
