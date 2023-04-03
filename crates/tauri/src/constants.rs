use strum_macros::{AsRefStr, Display, EnumString};

pub const INPUT_WIDTH: f64 = 640.0;
pub const INPUT_Y: f64 = 128.0;

// Min window size for settings panel
pub const MIN_WINDOW_WIDTH: f64 = 704.0;
pub const MIN_WINDOW_HEIGHT: f64 = 480.0;

// Check for a new version every 6 hours. 60 seconds * 60 minutes * 6 hours
pub const VERSION_CHECK_INTERVAL_S: u64 = 60 * 60 * 6;
// Check on start & every day for new lenses
pub const LENS_UPDATE_CHECK_INTERVAL_S: u64 = 60 * 60 * 24;

pub const SEARCH_WIN_NAME: &str = "main";
pub const SETTINGS_WIN_NAME: &str = "settings_window";
pub const STARTUP_WIN_NAME: &str = "startup_window";
pub const UPDATE_WIN_NAME: &str = "update_window";
pub const WIZARD_WIN_NAME: &str = "wizard_window";
pub const PROGRESS_WIN_NAME: &str = "progress_window";

#[derive(Display, EnumString, AsRefStr)]
pub enum TabLocation {
    #[strum(serialize = "/settings/connections")]
    Connections,
    #[strum(serialize = "/settings/discover")]
    Discover,
    #[strum(serialize = "/settings/library")]
    Library,
    #[strum(serialize = "/settings/plugins")]
    PluginSettings,
    #[strum(serialize = "/settings/user")]
    UserSettings,
}
