use strum_macros::Display;

#[derive(Display)]
pub enum ClientEvent {
    ClearSearch,
    FocusWindow,
    RefreshLensManager,
    RefreshPluginManager,
    RefreshSearchResults,
}
