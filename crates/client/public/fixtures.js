export let invoke = async (func_name, params) => {
    console.log(`calling: ${func_name} w/`, params);

    if  (func_name == "search_docs") {
        return [{
            doc_id: "123",
            domain: "google.com",
            title: "This is an example title",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            url: "https://google.com/this/is/a/path",
            score: 1.0
        }, {
            doc_id: "123",
            domain: "example.com",
            title: "This is an example title",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            url: "https://example.com/this/is/a/path",
            score: 1.0
        }];
    } else if (func_name == "search_lenses") {
        return [{
            author: "a5huynh",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            title: "fake_lense",
            html_url: null,
            download_url: null,
        }, {
            author: "a5huynh",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            title: "fake_lense_2_boogaloo",
            html_url: null,
            download_url: null,
        }];
    } else if (func_name == "list_installed_lenses") {
        return [{
            author: "a5huynh",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            title: "fake_lense",
            html_url: null,
            download_url: null,
        }, {
            author: "a5huynh",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            title: "fake_lense_2_boogaloo",
            html_url: null,
            download_url: null,
        }];
    } else if (func_name == "list_installable_lenses") {
        return [{
            author: "testing",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            name: "2007scape",
            sha: "12345678990",
            download_url: "",
            html_url: "",
        }, {
            author: "testing",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            name: "2007scape",
            sha: "12345678990",
            download_url: "",
            html_url: "",
        }, {
            author: "testing",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            name: "2007scape",
            sha: "12345678990",
            download_url: "",
            html_url: "",
        }, {
            author: "testing",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            name: "2007scape",
            sha: "12345678990",
            download_url: "",
            html_url: "",
        }, {
            author: "testing",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            name: "2007scape",
            sha: "12345678990",
            download_url: "",
            html_url: "",
        }];
    } else if (func_name == "list_plugins") {
        return [{
            author: "a5huynh",
            title: "chrome-exporter",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            is_enabled: true,
        }];
    } else if (func_name == "crawl_stats") {
        return {
            by_domain: [
                ['oldschool.runescape.wiki', { num_queued: 0, num_processing: 0, num_completed: 31413, num_indexed: 35453 }],
                ['en.wikipedia.org', { num_queued: 0, num_processing: 0, num_completed: 31413, num_indexed: 35453 }]
            ]
        };
    } else if (func_name == "load_user_settings") {
        return {
            "user.data_directory": {
                label: "Data Directory",
                value: "/Users/a5huynh/Library/Application Support/com.athlabs.spyglass-dev",
                form_type: "Text",
                help_text: "The data directory is where your index, lenses, plugins, and logs are stored. This will require a restart.",
            }
        };
    }

    return [];
};

export let listen = async () => {
    return {};
};

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