let invoke = () => {};
let listen = () => {};
if (window.__TAURI__) {
    invoke = window.__TAURI__.invoke;
    listen = window.__TAURI__.event.listen;
}

export async function onClearSearch(callback) {
    await listen('clear_search', callback);
}

export async function invokeSearch(query) {
    return await invoke("search", { query });
}