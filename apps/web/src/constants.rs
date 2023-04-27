// todo: pull these from environment variables? config?
#[cfg(not(debug_assertions))]
pub const HTTP_ENDPOINT: &str = "https://api.spyglass.fyi";
#[cfg(debug_assertions)]
pub const HTTP_ENDPOINT: &str = "http://127.0.0.1:8757";
