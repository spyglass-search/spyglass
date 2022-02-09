const invoke = window.__TAURI__.invoke;

export async function invokeSearch(query) {
    return await invoke("search", { query });
}