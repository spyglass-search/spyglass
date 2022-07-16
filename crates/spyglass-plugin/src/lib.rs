pub fn log() {
    unsafe { plugin_log() }
}

#[link(wasm_import_module = "spyglass")]
extern "C" {
    fn plugin_log();
}