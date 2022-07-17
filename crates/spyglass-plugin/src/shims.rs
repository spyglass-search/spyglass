use serde::de::DeserializeOwned;
use std::io;

pub fn log(msg: String) {
    println!("{}", msg);
    unsafe { plugin_log() }
}

#[link(wasm_import_module = "spyglass")]
extern "C" {
    fn plugin_log();
}

#[doc(hidden)]
pub fn object_from_stdin<T: DeserializeOwned>() -> Result<T, ron::Error> {
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap();
    ron::from_str(&buf)
}
