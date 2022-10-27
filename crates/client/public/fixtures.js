let CALLBACKS = {};

// This is used to mock endpoints when running the client headless via
// make run-headless-client
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
    } else if (func_name == "list_connections") {
        return [{
            id: "api.github.com",
            label: "GitHub",
            description: "Search through your GitHub repositories, starred repositories, and follows",
            scopes: [],
            is_connected: true,
        }, {
            id: "api.google.com",
            label: "Google Services",
            description: "Adds indexing support for Google services. This includes Gmail, Google Drive documents, and Google Calendar events",
            scopes: [],
            is_connected: false,
        }, {
            id: "api.examples.com",
            label: "Error Test",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            scopes: [],
            is_connected: false,
        }];
    } else if (func_name == "plugin:lens-updater|list_installed_lenses") {
        return [{
            author: "a5huynh",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            title: "fake_lense",
            hash: "",
            html_url: null,
            download_url: null,
        }, {
            author: "a5huynh",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            title: "fake_lense_2_boogaloo",
            hash: "",
            html_url: null,
            download_url: null,
        }];
    } else if (func_name == "plugin:lens-updater|list_installable_lenses") {
        return [{
            author: "a5huynh",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            name: "installable",
            sha: "fake-sha",
            html_url: "https://example.com",
            download_url: "https://example.com",
        }, {
            author: "a5huynh",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            name: "installable-two",
            sha: "fake-sha-1",
            html_url: "https://example.com",
            download_url: "https://example.com",
        }];
    } else if (func_name == "install_lens") {
        window.setTimeout(() => {
            CALLBACKS["RefreshLensManager"]();
        }, 5000);
    } else if (func_name == "list_plugins") {
        return [{
            author: "a5huynh",
            title: "chrome-exporter",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            is_enabled: true,
        }, {
            author: "a5huynh",
            title: "local-file-indexer",
            description: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
            is_enabled: false,
        }];
    } else if (func_name == "crawl_stats") {
        return {
            by_domain: [
                ['oldschool.runescape.wiki', { num_queued: 0, num_processing: 0, num_completed: 31413, num_indexed: 35453 }],
                ['en.wikipedia.org', { num_queued: 0, num_processing: 0, num_completed: 31413, num_indexed: 35453 }]
            ]
        };
    } else if (func_name == "load_user_settings") {
        return [
            ["_.data_directory", {
                label: "Data Directory",
                value: "/Users/a5huynh/Library/Application Support/com.athlabs.spyglass-dev",
                form_type: "Text",
                help_text: "The data directory is where your index, lenses, plugins, and logs are stored. This will require a restart.",
            }],
            ["_.autolaunch", {
                label: "Disable Autolaunch",
                value: "false",
                form_type: "Bool",
                help_text: "Prevents Spyglass from automatically launching when your computer first starts up.",
            }],
            ["_.disable_telemetry", {
                label: "Disable Telemetry",
                value: "false",
                form_type: "Bool",
                help_text: "Stop sending data to any 3rd-party service. See https://spyglass.fyi/telemetry for more info.",
            }],
            ["chrome-importer.CHROME_DATA_FOLDER", {
                label: "Chrome Data Folder",
                value: "",
                form_type: "Text",
                help_text: "",
            }],
            ["local-file-indexer.FOLDERS_LIST", {
                label: "Folders List",
                value: "[\"/Users/a5huynh/Documents\", \"/Users/a5huynh/Andrew's Vault\"]",
                form_type: "PathList",
                help_text: "List of folders that will be crawled & indexed. These folders will be crawled recursively, so you only need to specifiy the parent folder.",
            }],
            ["local-file-indexer.EXTS_LIST", {
                label: "File Types",
                value: "[\"md\", \"txt\"]",
                form_type: "StringList",
                help_text: "List of file types to index.",
            }]
        ];
    } else if (func_name == "plugin:tauri-plugin-startup|get_startup_progress") {
        return "Reticulating splines...";
    } else if (func_name == "authorize_connection") {
        if (params.name == "Google") {
            await new Promise(r => setTimeout(r, 5000));
        } else if (params.name == "Error Test") {
            await new Promise(r => setTimeout(r, 5000));
            throw 'Unable to connect';
        }

        return [];
    }

    return [];
};

export let listen = async (event, callback) => {
    console.log(`listen called w/ ${event}`);
    CALLBACKS[event] = callback;
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

export async function open_folder_path(path) {
    return await invoke('open_folder_path', { path });
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