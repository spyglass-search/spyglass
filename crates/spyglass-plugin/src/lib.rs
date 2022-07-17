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
        pub fn request_queue() {
            STATE.with(|state| {
                state.borrow_mut().request_queue();
            })
        }
    };
}
pub trait SpyglassPlugin {
    fn load(&self);
    fn request_queue(&self);
}

#[derive(Deserialize, Serialize)]
pub struct PluginMountRequest {
    pub dst: String,
    pub src: String,
}

#[derive(Deserialize, Serialize)]
pub struct PluginEnqueueRequest {
    pub url: String,
}
