use spyglass_plugin::*;

#[derive(Default)]
struct Plugin;

register_plugin!(Plugin);

impl SpyglassPlugin for Plugin {
    fn load(&self) {
        log("plugin load".into());
    }

    fn request_queue(&self) {
        log("request_queue".into());
    }
}
