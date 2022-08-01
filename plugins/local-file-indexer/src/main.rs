use spyglass_plugin::*;

#[derive(Default)]
struct Plugin;

register_plugin!(Plugin);

impl SpyglassPlugin for Plugin {
    fn load(&self) {}
    fn update(&self) {}
}
