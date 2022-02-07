const invoke = window.__TAURI__.invoke;

export async function invokeSearch(name) {
    return await invoke("search", { query: "test query" });
}