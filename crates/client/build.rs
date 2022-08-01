// Example custom build script.
fn main() {
    // Tell Cargo that if the given file changes, to rerun this build script.
    let is_headless = option_env!("HEADLESS_CLIENT");
    if is_headless.is_some() {
        println!("cargo:rustc-cfg=headless");
    }
}
