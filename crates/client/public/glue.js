let invoke = () => {};
let listen = () => {};
if (window.__TAURI__) {
    invoke = window.__TAURI__.invoke;
    listen = window.__TAURI__.event.listen;
}

export async function escape() {
    return await invoke("escape");
}

export async function onClearSearch(callback) {
    await listen('clear_search', callback);
}

export async function searchDocs(lenses, query) {
    return await invoke("search", { lenses, query });
}

export async function searchLenses(query) {
    return await invoke("search_lenses", { query });
}

export async function openResult(url) {
    return await invoke("open_result", { url });
}

export function resizeWindow(height) {
    return invoke("resize_window", { height });
}