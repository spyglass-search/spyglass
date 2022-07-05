let invoke = () => {};
let listen = () => {};
if (window.__TAURI__) {
    invoke = window.__TAURI__.invoke;
    listen = window.__TAURI__.event.listen;
}

export async function escape() {
    return await invoke('escape');
}

export async function onClearSearch(callback) {
    await listen('clear_search', callback);
}

export async function onFocus(callback) {
    await listen('focus_window', callback);
}

export async function onRefreshResults(callback) {
    await listen('refresh_results', callback);
}

export async function on_refresh_lens_manager(callback) {
    await listen('refresh_lens_manager', callback);
}

export async function crawlStats() {
    return await invoke('crawl_stats');
}

export async function deleteDoc(id) {
    return await invoke('delete_doc', { id });
}

export async function install_lens(downloadUrl) {
    return await invoke('install_lens', { downloadUrl })
}

export async function listInstalledLenses() {
    return await invoke('list_installed_lenses');
}

export async function listInstallableLenses() {
    return await invoke('list_installable_lenses');
}

export async function network_change(isOffline) {
    return await invoke('network_change', { isOffline });
}

export async function recrawl_domain(domain) {
    return await invoke('recrawl_domain', { domain });
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

export async function openLensFolder() {
    return await invoke('open_lens_folder');
}

export async function resizeWindow(height) {
    return await invoke('resize_window', { height });
}