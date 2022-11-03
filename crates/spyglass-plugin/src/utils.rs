use std::path::PathBuf;
use url::Url;

// Create a file URI
pub fn path_to_uri(path: PathBuf) -> String {
    let path_str = path.to_str().expect("Unable to convert path to string");

    // Eventually this will be away to keep track of multiple devices and searching across
    // them. Might make sense to generate a UUID and assign to this computer(?) hostname
    // can be changed by the user.
    let host = if let Ok(hname) = std::env::var("HOST_NAME") {
        hname
    } else {
        "home.local".into()
    };

    let mut new_url = Url::parse("file://").expect("Base URI");
    let _ = new_url.set_host(Some(&host));
    // Fixes issues handling windows drive letters
    new_url.set_path(&path_str.replace(':', "%3A"));
    new_url.to_string()
}
