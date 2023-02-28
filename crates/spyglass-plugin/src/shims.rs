use serde::{de::DeserializeOwned, Serialize};
use std::io;

use crate::{DocumentQuery, PluginCommandRequest, TagModification};
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

/// Add an item to the Spyglass crawl queue
pub fn enqueue_all(urls: &[String]) {
    if object_to_stdout(&PluginCommandRequest::Enqueue { urls: urls.into() }).is_ok() {
        unsafe {
            plugin_cmd();
        }
    }
}

/// Utility function to log to spyglass logs
pub fn log(msg: String) {
    println!("{msg}");
    unsafe {
        plugin_log();
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

pub fn modify_tags(query: DocumentQuery, modification: TagModification) -> Result<(), ron::Error> {
    object_to_stdout(&PluginCommandRequest::ModifyTags {
        documents: query,
        tag_modifications: modification,
    })?;

    unsafe {
        plugin_cmd();
    }
    Ok(())
}

pub fn subscribe_for_documents(query: DocumentQuery) -> Result<(), ron::Error> {
    object_to_stdout(&PluginCommandRequest::QueryDocuments {
        query,
        subscribe: true,
    })?;

    unsafe {
        plugin_cmd();
    }
    Ok(())
}

pub fn query_documents(query: DocumentQuery) -> Result<(), ron::Error> {
    object_to_stdout(&PluginCommandRequest::QueryDocuments {
        query,
        subscribe: false,
    })?;

    unsafe {
        plugin_cmd();
    }
    Ok(())
}
