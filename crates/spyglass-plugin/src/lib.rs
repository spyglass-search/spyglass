mod shims;
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
