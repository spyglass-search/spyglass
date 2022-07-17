use wasmer::{Exports, Function, Store};
use wasmer_wasi::WasiEnv;

use super::{wasi_read_string, PluginConfig, PluginEnv};

pub fn register_exports(plugin: &PluginConfig, store: &Store, env: &WasiEnv) -> Exports {
    let mut exports = Exports::new();

    let log_func = Function::new_native_with_env(
        store,
        PluginEnv {
            name: plugin.name.clone(),
            wasi_env: env.clone(),
        },
        plugin_log,
    );

    exports.insert("plugin_log", log_func);

    exports
}

pub(crate) fn plugin_log(env: &PluginEnv) {
    if let Ok(msg) = wasi_read_string(&env.wasi_env) {
        log::info!("{}: {}", env.name, msg);
    }
}
