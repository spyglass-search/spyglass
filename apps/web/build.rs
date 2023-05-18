// build.rs
use std::process::Command;
fn main() {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();
    let git_hash = if let Ok(Ok(output)) = output.map(|x| String::from_utf8(x.stdout)) {
        output
    } else {
        String::from("N/A")
    };

    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
}
