use strum_macros::Display;

#[derive(Display)]
pub enum ClientEvent {
    ClearSearch,
    FocusWindow,
    RefreshLensManager,
    RefreshPluginManager,
    RefreshSearchResults,
}

#[derive(Display)]
pub enum ClientInvoke {
    #[strum(serialize = "open_plugins_folder")]
    EditPluginSettings,
}
