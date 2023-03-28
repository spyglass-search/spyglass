use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, Display};

#[derive(Clone, Debug, Deserialize)]
pub struct ListenPayload<T> {
    pub payload: T,
}

#[derive(AsRefStr, Display)]
pub enum ClientEvent {
    ClearSearch,
    FocusWindow,
    FolderChosen,
    LLMResponse,
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

#[derive(AsRefStr, Display)]
pub enum ClientInvoke {
    #[strum(serialize = "authorize_connection")]
    AuthorizeConnection,
    #[strum(serialize = "choose_folder")]
    ChooseFolder,
    #[strum(serialize = "default_indices")]
    DefaultIndices,
    #[strum(serialize = "escape")]
    Escape,
    #[strum(serialize = "open_plugins_folder")]
    EditPluginSettings,
    #[strum(serialize = "get_library_stats")]
    GetLibraryStats,
    #[strum(serialize = "get_shortcut")]
    GetShortcut,
    #[strum(serialize = "plugin:tauri-plugin-startup|get_startup_progress")]
    GetStartupProgressText,
    #[strum(serialize = "plugin:lens-updater|install_lens")]
    InstallLens,
    #[strum(serialize = "list_connections")]
    ListConnections,
    #[strum(serialize = "plugin:lens-updater|list_installed_lenses")]
    ListInstalledLenses,
    #[strum(serialize = "plugin:lens-updater|list_installable_lenses")]
    ListInstallableLenses,
    #[strum(serialize = "list_plugins")]
    ListPlugins,
    #[strum(serialize = "load_user_settings")]
    LoadUserSettings,
    #[strum(serialize = "load_action_settings")]
    LoadUserActions,
    #[strum(serialize = "resync_connection")]
    ResyncConnection,
    #[strum(serialize = "revoke_connection")]
    RevokeConnection,
    #[strum(serialize = "toggle_plugin")]
    TogglePlugin,
    #[strum(serialize = "plugin:lens-updater|run_lens_updater")]
    RunLensUpdater,
    #[strum(serialize = "open_folder_path")]
    OpenFolder,
    #[strum(serialize = "open_lens_folder")]
    OpenLensFolder,
    #[strum(serialize = "open_result")]
    OpenResult,
    #[strum(serialize = "copy_to_clipboard")]
    CopyToClipboard,
    #[strum(serialize = "open_settings_folder")]
    OpenSettingsFolder,
    #[strum(serialize = "plugin:lens-updater|uninstall_lens")]
    UninstallLens,
    #[strum(serialize = "update_and_restart")]
    UpdateAndRestart,
    #[strum(serialize = "wizard_finished")]
    WizardFinished,
    #[strum(serialize = "navigate")]
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

#[derive(Deserialize, Serialize)]
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
pub struct WizardFinishedParams {
    #[serde(rename(serialize = "toggleFileIndexer"))]
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
pub struct LLMResultPayload {
    pub token: String,
}
