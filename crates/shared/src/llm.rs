use serde::Serialize;

// Generation is roughly the order things happen.
pub enum ChatStream {
    LoadingPrompt,
    ChatStart,
    Token(String),
    ChatDone,
}

#[derive(Serialize)]
pub enum ChatRole {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

#[derive(Serialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Serialize)]
pub struct LlmSession {
    pub messages: Vec<ChatMessage>,
}
