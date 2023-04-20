use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub enum RpcEventType {
    ConnectionSyncFinished,
    LensUninstalled,
    LensInstalled,
    LLMResponse,
    ModelDownloadStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RpcEvent {
    /// Event Type
    pub event_type: RpcEventType,
    /// Payload serialized as JSON if applicable.
    pub payload: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ModelDownloadStatusPayload {
    Finished { model_name: String },
    Error { model_name: String, msg: String },
    InProgress { model_name: String, percent: u8 },
}
