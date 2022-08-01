pub mod consts;
mod shims;
use serde::{Deserialize, Serialize};
pub use shims::*;

#[macro_export]
macro_rules! register_plugin {
    ($t:ty) => {
        thread_local! {
            static STATE: std::cell::RefCell<$t> = std::cell::RefCell::new(Default::default());
        }

        fn main() {
            STATE.with(|state| {
                state.borrow_mut().load();
            });
        }

        #[no_mangle]
        pub fn update() {
            STATE.with(|state| {
                state.borrow_mut().update();
            })
        }
    };
}
pub trait SpyglassPlugin {
    /// Initial plugin load, setup any configuration you need here as well as
    /// subscribe to specific events.
    fn load(&self);
    /// Request plugin for updates
    fn update(&self);
}

#[derive(Deserialize, Serialize)]
pub enum PluginEvent {
    /// Check for updates at a fixed interval
    CheckUpdateInterval,
}

#[derive(Deserialize, Serialize)]
pub enum PluginCommandRequest {
    // Enqueue a list of URLs into the crawl queue
    Enqueue { urls: Vec<String> },
    // List the contents of a directory
    ListDir(String),
    // Subscribe to PluginEvents
    Subscribe(PluginEvent),
    // Run a sqlite query on a db file. NOTE: This is a workaround due to the fact
    // that sqlite can not be easily compiled to wasm... yet!
    SqliteQuery { path: String, query: String },
    // Request mounting a file & its contents to the plugin VFS
    SyncFile { dst: String, src: String },
}
