use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, Display};
use ts_rs::TS;

/// NOTE: When adding a new invoke command,
/// the label should match up to the tauri generated command names.
#[derive(AsRefStr, Display, Deserialize, Serialize, TS)]
#[ts(export)]
pub enum ClientInvoke {
    #[serde(rename = "authorize_connection")]
    AuthorizeConnection,
    #[serde(rename = "choose_folder")]
    ChooseFolder,
    #[serde(rename = "copy_to_clipboard")]
    CopyToClipboard,
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
    #[serde(rename = "open_big_mode")]
    OpenBigMode,
    #[serde(rename = "open_folder_path")]
    OpenFolder,
    #[serde(rename = "open_lens_folder")]
    OpenLensFolder,
    #[serde(rename = "open_result")]
    OpenResult,
    #[serde(rename = "open_settings_folder")]
    OpenSettingsFolder,
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
    #[serde(rename = "uninstall_lens")]
    UninstallLens,
    #[serde(rename = "update_and_restart")]
    UpdateAndRestart,
    #[serde(rename = "wizard_finished")]
    WizardFinished,
    #[serde(rename = "navigate")]
    Navigate,
}
