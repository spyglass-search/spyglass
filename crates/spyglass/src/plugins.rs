use wasmer::{Store, Module, Instance};
use wasmer_wasi::WasiState;

pub fn test_plugin() -> anyhow::Result<()>{
    let store = Store::default();
    let module = Module::from_file(&store, "plugins/test-plugin/hello.wasm")?;

    // Create the `WasiEnv`
    let mut wasi_env = WasiState::new("command-name")
        .args(&["Gordon"])
        .finalize()?;

    // Generate an `ImportObject`
    let import_object = wasi_env.import_object(&module)?;

    // Insantiate the module wn the imports
    let instance = Instance::new(&module, &import_object)?;

    // Lets call the `_start` function, which is our `main` function in Rust
    let start = instance.exports.get_function("_start")?;
    start.call(&[])?;

    Ok(())
}