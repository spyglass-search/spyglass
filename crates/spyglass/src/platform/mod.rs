use std::path::Path;

use anyhow::anyhow;

/// Place for platform specific code.

pub fn linux_open(url: &str) -> anyhow::Result<()> {
    // https://docs.appimage.org/packaging-guide/environment-variables.html
    let appimage = std::env::var_os("APPIMAGE");
    let owd = std::env::var_os("OWD").unwrap_or_default();

    // Taken from the `current_binary` code in Tauri to pull the correct parent folder
    // for the binary.
    if appimage.is_some() {
        let parent = Path::new(&owd);
        match std::process::Command::new("xdg-open")
            .arg(url)
            .current_dir(parent)
            .output()
        {
            Ok(_) => Ok(()),
            Err(err) => Err(anyhow!(err.to_string())),
        }
    } else {
        let _ = open::that(url);
        Ok(())
    }
}
