use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub enum RpcEventType {
    ChatStream,
    ConnectionSyncFinished,
    LensUninstalled,
    LensInstalled,
    ModelDownloadStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RpcEvent {
    /// Event Type
    pub event_type: RpcEventType,
    /// Payload serialized as JSON if applicable.
    pub payload: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ModelDownloadStatusPayload {
    Finished { model_name: String },
    Error { model_name: String, msg: String },
    InProgress { model_name: String, percent: u8 },
}
