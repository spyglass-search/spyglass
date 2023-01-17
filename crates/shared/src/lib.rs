pub mod config;
pub mod constants;
pub mod event;
pub mod form;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod plugin;
pub mod regex;
pub mod request;
pub mod response;

/// A platform-agnostic way to turn a URL file path into something that can
/// be opened & crawled.
pub fn url_to_file_path(path: &str, is_windows: bool) -> String {
    // Unescape colons & spaces
    let mut path = path.replace("%3A", ":").replace("%20", " ");
    // Strip superfluous path prefix
    if is_windows {
        path = path
            .strip_prefix('/')
            .map(|s| s.to_string())
            .unwrap_or(path);
        // Convert path dividers into Windows specific ones.
        path = path.replace('/', "\\");
    }

    path
}
