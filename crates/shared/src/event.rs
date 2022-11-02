use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, Display};

#[derive(Debug, Deserialize)]
pub struct ListenPayload {
    pub payload: String,
}

#[derive(AsRefStr, Display)]
pub enum ClientEvent {
    ClearSearch,
    FocusWindow,
    FolderChosen,
    Navigate,
    RefreshConnections,
    RefreshLensManager,
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
    #[strum(serialize = "escape")]
    Escape,
    #[strum(serialize = "open_plugins_folder")]
    EditPluginSettings,
    #[strum(serialize = "crawl_stats")]
    GetCrawlStats,
    #[strum(serialize = "plugin:tauri-plugin-startup|get_startup_progress")]
    GetStartupProgressText,
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
    #[strum(serialize = "resync_connection")]
    ResyncConnection,
    #[strum(serialize = "revoke_connection")]
    RevokeConnection,
    #[strum(serialize = "plugin:lens-updater|run_lens_updater")]
    RunLensUpdater,
    #[strum(serialize = "open_folder_path")]
    OpenFolder,
    #[strum(serialize = "open_lens_folder")]
    OpenLensFolder,
    #[strum(serialize = "open_settings_folder")]
    OpenSettingsFolder,
    #[strum(serialize = "update_and_restart")]
    UpdateAndRestart,
}

#[derive(Deserialize, Serialize)]
pub struct AuthorizeConnectionParams {
    pub id: String,
}
