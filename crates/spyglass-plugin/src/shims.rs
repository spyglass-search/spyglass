use serde::{de::DeserializeOwned, Serialize};
use std::io;

use crate::{ListDirEntry, PluginCommandRequest, PluginSubscription};

pub fn delete_doc(url: &str) {
    if object_to_stdout(&PluginCommandRequest::DeleteDoc {
        url: url.to_string(),
    })
    .is_ok()
    {
        unsafe {
            plugin_cmd();
        }
    }
}

pub fn subscribe(event: PluginSubscription) {
    if object_to_stdout(&PluginCommandRequest::Subscribe(event)).is_ok() {
        unsafe {
            plugin_cmd();
        }
    }
}

/// Add an item to the Spyglass crawl queue
pub fn enqueue_all(urls: &[String]) {
    if object_to_stdout(&PluginCommandRequest::Enqueue { urls: urls.into() }).is_ok() {
        unsafe {
            plugin_cmd();
        }
    }
}

/// List dir
pub fn list_dir(path: &str) -> Result<Vec<ListDirEntry>, ron::error::SpannedError> {
    if object_to_stdout(&PluginCommandRequest::ListDir {
        path: path.to_string(),
    })
    .is_ok()
    {
        unsafe {
            plugin_cmd();
        }
        return object_from_stdin::<Vec<ListDirEntry>>();
    }

    Ok(Vec::new())
}

/// Utility function to log to spyglass logs
pub fn log(msg: String) {
    println!("{}", msg);
    unsafe {
        plugin_log();
    }
}

/// Hacky workaround until rusqlite can compile to wasm easily.
/// Path is expected to be rooted in the plugins data directory.
pub fn sqlite3_query(path: &str, query: &str) -> Result<Vec<String>, ron::error::SpannedError> {
    if object_to_stdout(&PluginCommandRequest::SqliteQuery {
        path: path.to_string(),
        query: query.to_string(),
    })
    .is_ok()
    {
        unsafe { plugin_cmd() };
        return object_from_stdin::<Vec<String>>();
    }

    Ok(Vec::new())
}

/// Adds / updates a file in the plugin VFS from the host.
pub fn sync_file(dst: String, src: String) {
    if object_to_stdout(&PluginCommandRequest::SyncFile { dst, src }).is_ok() {
        unsafe {
            plugin_cmd();
        }
    }
}

#[link(wasm_import_module = "spyglass")]
extern "C" {
    fn plugin_cmd();
    fn plugin_log();
}

#[doc(hidden)]
pub fn object_from_stdin<T: DeserializeOwned>() -> Result<T, ron::error::SpannedError> {
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap();
    ron::from_str(&buf)
}

#[doc(hidden)]
pub fn object_to_stdout(obj: &impl Serialize) -> Result<(), ron::Error> {
    println!("{}", ron::ser::to_string(obj)?);
    Ok(())
}
