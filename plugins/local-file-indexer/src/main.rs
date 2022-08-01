use spyglass_plugin::*;

#[derive(Default)]
struct Plugin;

register_plugin!(Plugin);

impl SpyglassPlugin for Plugin {
    fn load(&self) {
        if let Ok(entries) = list_dir("/Users/a5huynh/Documents/projects/blog/blog-src", true) {
            for path in entries {
                log(path);
            }
        }
    }
    fn update(&self) {}
}
