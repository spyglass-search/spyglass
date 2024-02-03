// build.rs
use std::{
    path::{Path, PathBuf},
    process::Command,
};

// https://stackoverflow.com/questions/43577885/is-there-a-cargo-environment-variable-for-the-workspace-directory
fn workspace_dir() -> PathBuf {
    let output = std::process::Command::new(env!("CARGO"))
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .unwrap()
        .stdout;
    let cargo_path = Path::new(std::str::from_utf8(&output).unwrap().trim());
    cargo_path.parent().unwrap().to_path_buf()
}

fn main() {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();
    let git_hash = if let Ok(Ok(output)) = output.map(|x| String::from_utf8(x.stdout)) {
        output
    } else {
        String::from("N/A")
    };

    let workspace = workspace_dir();

    // Check for the .env file, and if not present, copy in the template.
    if !workspace.join(".env").exists() {
        std::fs::copy(workspace.join(".env.template"), workspace.join(".env")).unwrap();
    }

    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
}
