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

#[derive(AsRefStr, Display, Deserialize, Serialize, TS)]
#[ts(export)]
pub enum ClientInvoke {
    #[serde(rename = "authorize_connection")]
    AuthorizeConnection,
    #[serde(rename = "choose_folder")]
    ChooseFolder,
    #[serde(rename = "default_indices")]
    DefaultIndices,
    #[serde(rename = "escape")]
    Escape,
    #[serde(rename = "open_plugins_folder")]
    EditPluginSettings,
    #[serde(rename = "get_library_stats")]
    GetLibraryStats,
    #[serde(rename = "get_shortcut")]
    GetShortcut,
    #[serde(rename = "get_startup_progress")]
    GetStartupProgressText,
    #[serde(rename = "install_lens")]
    InstallLens,
    #[serde(rename = "list_connections")]
    ListConnections,
    #[serde(rename = "list_installed_lenses")]
    ListInstalledLenses,
    #[serde(rename = "list_installable_lenses")]
    ListInstallableLenses,
    #[serde(rename = "list_plugins")]
    ListPlugins,
    #[serde(rename = "load_user_settings")]
    LoadUserSettings,
    #[serde(rename = "load_action_settings")]
    LoadUserActions,
    #[serde(rename = "resize_window")]
    ResizeWindow,
    #[serde(rename = "resync_connection")]
    ResyncConnection,
    #[serde(rename = "revoke_connection")]
    RevokeConnection,
    #[serde(rename = "run_lens_updater")]
    RunLensUpdater,
    #[serde(rename = "save_user_settings")]
    SaveUserSettings,
    #[serde(rename = "search_docs")]
    SearchDocuments,
    #[serde(rename = "search_lenses")]
    SearchLenses,
    #[serde(rename = "open_folder_path")]
    OpenFolder,
    #[serde(rename = "open_lens_folder")]
    OpenLensFolder,
    #[serde(rename = "open_result")]
    OpenResult,
    #[serde(rename = "copy_to_clipboard")]
    CopyToClipboard,
    #[serde(rename = "open_settings_folder")]
    OpenSettingsFolder,
    #[serde(rename = "uninstall_lens")]
    UninstallLens,
    #[serde(rename = "update_and_restart")]
    UpdateAndRestart,
    #[serde(rename = "wizard_finished")]
    WizardFinished,
    #[serde(rename = "navigate")]
    Navigate,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelStatusPayload {
    pub msg: String,
    pub percent: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelStatusPayloadWrapper {
    pub payload: ModelStatusPayload,
}
