use strum_macros::{AsRefStr, Display};

#[derive(AsRefStr, Display)]
pub enum ClientEvent {
    ClearSearch,
    FocusWindow,
    Navigate,
    RefreshLensManager,
    RefreshPluginManager,
    RefreshSearchResults,
    StartupProgress,
}

#[derive(AsRefStr, Display)]
pub enum ClientInvoke {
    #[strum(serialize = "escape")]
    Escape,
    #[strum(serialize = "open_plugins_folder")]
    EditPluginSettings,
    #[strum(serialize = "crawl_stats")]
    GetCrawlStats,
    #[strum(serialize = "plugin:tauri-plugin-startup|get_startup_progress")]
    GetStartupProgressText,
    #[strum(serialize = "plugin:lens-updater|list_installed_lenses")]
    ListInstalledLenses,
    #[strum(serialize = "plugin:lens-updater|list_installable_lenses")]
    ListInstallableLenses,
    #[strum(serialize = "list_plugins")]
    ListPlugins,
    #[strum(serialize = "load_user_settings")]
    LoadUserSettings,
    #[strum(serialize = "plugin:lens-updater|run_lens_updater")]
    RunLensUpdater,
    #[strum(serialize = "open_lens_folder")]
    OpenLensFolder,
    #[strum(serialize = "open_settings_folder")]
    OpenSettingsFolder,
    #[strum(serialize = "update_and_restart")]
    UpdateAndRestart,
}
