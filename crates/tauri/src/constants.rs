pub const INPUT_WIDTH: f64 = 640.0;
pub const INPUT_Y: f64 = 128.0;

// Check for a new version every 6 hours. 60 seconds * 60 minutes * 6 hours
pub const VERSION_CHECK_INTERVAL_S: u64 = 60 * 60 * 6;

pub const APP_USER_AGENT: &str = "spyglass (github.com/a5huynh/spyglass)";
pub const DISCORD_JOIN_URL: &str = "https://discord.gg/663wPVBSTB";
pub const LENS_DIRECTORY_INDEX_URL: &str =
    "https://raw.githubusercontent.com/spyglass-search/lens-box/main/index.ron";

pub const STATS_WIN_NAME: &str = "crawl_stats";
pub const LENS_MANAGER_WIN_NAME: &str = "lens_manager";
pub const PLUGIN_MANAGER_WIN_NAME: &str = "plugin_manager";
