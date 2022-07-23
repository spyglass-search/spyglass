use strum_macros::{AsRefStr, Display};

#[derive(AsRefStr, Display)]
pub enum ClientEvent {
    ClearSearch,
    FocusWindow,
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
}
