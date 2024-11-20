use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, Display};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize)]
pub struct ListenPayload<T> {
    pub payload: T,
}

#[derive(AsRefStr, Display, TS)]
#[ts(export)]
pub enum ClientEvent {
    ClearSearch,
    FocusWindow,
    FolderChosen,
    LensInstalled,
    LensUninstalled,
    Navigate,
    RefreshConnections,
    /// Request a refresh of the discover lens page when a lens is succesfully installed.
    RefreshDiscover,
    /// Request a refresh of the user lens library .
    RefreshLensLibrary,
    RefreshPluginManager,
    RefreshSearchResults,
    StartupProgress,
    UpdateLensFinished,
}

#[derive(Deserialize, Serialize)]
pub struct AuthorizeConnectionParams {
    pub id: String,
}

#[derive(Deserialize, Serialize)]
pub struct ResyncConnectionParams {
    pub id: String,
    pub account: String,
}

#[derive(Deserialize, Serialize)]
pub struct InstallLensParams {
    pub name: String,
}

#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct OpenResultParams {
    pub url: String,
    pub application: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct CopyContext {
    pub txt: String,
}

#[derive(Deserialize, Serialize)]
pub struct TogglePluginParams {
    pub name: String,
    pub enabled: bool,
}

#[derive(Deserialize, Serialize)]
pub struct UninstallLensParams {
    pub name: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WizardFinishedParams {
    pub toggle_audio_transcription: bool,
    pub toggle_file_indexer: bool,
}

#[derive(Deserialize, Serialize)]
pub struct NavigateParams {
    pub page: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ModelStatusPayload {
    pub msg: String,
    pub percent: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ModelStatusPayloadWrapper {
    pub payload: ModelStatusPayload,
}
