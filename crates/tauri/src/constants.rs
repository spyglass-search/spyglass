use strum_macros::{AsRefStr, Display};

pub const INPUT_WIDTH: f64 = 640.0;
pub const INPUT_Y: f64 = 128.0;

// Min window size for settings panel
pub const MIN_WINDOW_WIDTH: f64 = 704.0;
pub const MIN_WINDOW_HEIGHT: f64 = 480.0;

// Check for a new version every 6 hours. 60 seconds * 60 minutes * 6 hours
pub const VERSION_CHECK_INTERVAL_S: u64 = 60 * 60 * 6;
// Check on start & every day for new lenses
pub const LENS_UPDATE_CHECK_INTERVAL_S: u64 = 60 * 60 * 24;

#[derive(AsRefStr, Display)]
pub enum Windows {
    #[strum(serialize = "ask_clippy_window")]
    AskClippy,
    #[strum(serialize = "progress_window")]
    ProgressPopup,
    #[strum(serialize = "main")]
    SearchBar,
    #[strum(serialize = "settings_window")]
    Settings,
    #[strum(serialize = "startup_window")]
    Startup,
    #[strum(serialize = "update_window")]
    UpdatePopup,
    #[strum(serialize = "wizard_window")]
    Wizard,
}
