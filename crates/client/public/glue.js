let invoke = () => {};
let listen = () => {};
if (window.__TAURI__) {
    invoke = window.__TAURI__.invoke;
    listen = window.__TAURI__.event.listen;
}

export async function deleteDoc(id) {
    return await invoke('delete_doc', { id });
}

export async function delete_domain(domain) {
    return await invoke('delete_domain', { domain });
}

export async function install_lens(downloadUrl) {
    return await invoke('install_lens', { downloadUrl })
}

export async function network_change(isOffline) {
    return await invoke('network_change', { isOffline });
}

export async function recrawl_domain(domain) {
    return await invoke('recrawl_domain', { domain });
}

export async function save_user_settings(settings) {
    return await invoke('save_user_settings', { settings });
}

export async function searchDocs(lenses, query) {
    return await invoke('search_docs', { lenses, query });
}

export async function searchLenses(query) {
    return await invoke('search_lenses', { query });
}

export async function openResult(url) {
    return await invoke('open_result', { url });
}

export async function resizeWindow(height) {
    return await invoke('resize_window', { height });
}

export async function toggle_plugin(name) {
    return await invoke('toggle_plugin', { name })
}