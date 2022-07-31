use strum_macros::{AsRefStr, Display};

#[derive(AsRefStr, Display)]
pub enum ClientEvent {
    ClearSearch,
    FocusWindow,
    Navigate,
    RefreshLensManager,
    RefreshPluginManager,
    RefreshSearchResults,
}

#[derive(AsRefStr, Display)]
pub enum ClientInvoke {
    #[strum(serialize = "escape")]
    Escape,
    #[strum(serialize = "open_plugins_folder")]
    EditPluginSettings,
    #[strum(serialize = "crawl_stats")]
    GetCrawlStats,
    #[strum(serialize = "list_installed_lenses")]
    ListInstalledLenses,
    #[strum(serialize = "list_installable_lenses")]
    ListInstallableLenses,
    #[strum(serialize = "load_user_settings")]
    LoadUserSettings,
    #[strum(serialize = "open_lens_folder")]
    OpenLensFolder,
}
