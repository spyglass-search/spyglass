let invoke = () => {};
let listen = () => {};
if (window.__TAURI__) {
    invoke = window.__TAURI__.core.invoke;
    listen = window.__TAURI__.event.listen;
}

export async function deleteDoc(id) {
    return await invoke('delete_doc', { id });
}

export async function network_change(isOffline) {
    return await invoke('network_change', { isOffline });
}

export async function recrawl_domain(domain) {
    return await invoke('recrawl_domain', { domain });
}

export async function save_user_settings(settings, restart) {
    return await invoke('save_user_settings', { settings: Object.fromEntries(settings), restart });
}

export async function searchDocs(lenses, query) {
    return await invoke('search_docs', { lenses, query });
}

export async function searchLenses(query) {
    return await invoke('search_lenses', { query });
}

export async function open_folder_path(path) {
    return await invoke('open_folder_path', { path });
}

export async function resizeWindow(height) {
    return await invoke('resize_window', { height });
}