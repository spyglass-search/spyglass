use serde::{de::DeserializeOwned, Serialize};
use std::io;

use crate::{PluginEnqueueRequest, PluginMountRequest};

pub fn enqueue(url: String) {
    if object_to_stdout(&PluginEnqueueRequest { url }).is_ok() {
        unsafe { plugin_enqueue() }
    }
}

pub fn log(msg: String) {
    println!("{}", msg);
    unsafe { plugin_log() }
}

pub fn sync_file(dst: String, src: String) {
    if object_to_stdout(&PluginMountRequest { dst, src }).is_ok() {
        unsafe { plugin_sync_file() }
    }
}

#[link(wasm_import_module = "spyglass")]
extern "C" {
    fn plugin_enqueue();
    fn plugin_log();
    fn plugin_sync_file();
}

#[doc(hidden)]
pub fn object_from_stdin<T: DeserializeOwned>() -> Result<T, ron::Error> {
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap();
    ron::from_str(&buf)
}

#[doc(hidden)]
pub fn object_to_stdout(obj: &impl Serialize) -> Result<(), ron::Error> {
    println!("{}", ron::ser::to_string(obj)?);
    Ok(())
}
