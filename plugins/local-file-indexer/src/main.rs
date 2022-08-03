use spyglass_plugin::*;

#[derive(Default)]
struct Plugin;

register_plugin!(Plugin);

impl SpyglassPlugin for Plugin {
    fn load(&self) {
        // on load subscribe to file notif events for folders
        subscribe(PluginSubscription::WatchDirectory {
            path: "/Users/a5huynh/Documents/projects/blog/blog-src".to_string(),
            recurse: false,
        });
    }

    fn update(&self, event: PluginEvent) {
        log(format!("received event: {:?}", event));
    }
}
