let CALLBACKS = {};

// This is used to mock endpoints when running the client headless via
// make run-headless-client
export let invoke = async (func_name, params) => {
  console.log(`calling: ${func_name} w/`, params);

  if (func_name == "search_docs") {
    let meta =  {
      query: params.query,
      num_docs: 426552,
      wall_time_ms: 1234,
    };

    if (params.query == "blank") {
      return { meta, results: [] };
    }

    return {
      meta,
      results: [
        {
          doc_id: "123",
          domain: "google.com",
          title: "This is an example title",
          description:
            "<div>Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.</div>",
          crawl_uri: "https://google.com/this/is/a/path",
          url: "https://google.com/this/is/a/path",
          tags: [
            ["source", "web"],
            ["lens", "google"],
            ["lens", "search-engines"],
            ["favorited", "Favorited"],
          ],
          score: 1.0,
        },
        {
          doc_id: "123",
          domain: "localhost",
          title: "/Users/Blah/Documents/Special Information",
          description: "",
          crawl_uri: "file:///C%3A/Blah/Documents/Special%20Information",
          url: "file:///Users/Blah/Documents/Special%20Information",
          tags: [
            ["lens", "files"],
            ["type", "directory"],
          ],
          score: 1.0,
        },
        {
          doc_id: "123",
          domain: "drive.google.com",
          title:
            "This is an example super long title to demonstrate very long titles that go on for a very long time and then some.",
          description:
            "<div>Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.</div>",
          crawl_uri: "api://account@drive.google.com/1540812340985",
          url: "https://example.com",
          tags: [
            ["mimetype", "application/pdf"],
            ["source", "drive.google.com"],
            ["type", "file"],
            ["lens", "GDrive"],
            ["owner", "bob.dole@example.com"],
          ],
          score: 1.0,
        },
        {
          doc_id: "123",
          domain: "localhost",
          title: "README.md",
          description:
            "<div>Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.</div>",
          crawl_uri:
            "file://localhost/User/alice/Documents/Projects/personal/test-project/github-repos/blog/src/blah-blah/README.md",
          url: "file://localhost/User/alice/Documents/Projects/personal/test-project/github-repos/blog/src/blah-blah/README.md",
          tags: [
            ["mimetype", "application/pdf"],
            ["source", "localhost"],
            ["lens", "files"],
          ],
          score: 1.0,
        },
        {
          doc_id: "123",
          domain: "drive.google.com",
          title: "API Example Doc",
          description:
            "<div>Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.</div>",
          crawl_uri: "api://drive.google.com/24938aslkdj-313-19384",
          url: "https://example.com/this/is/a/path",
          tags: [
            ["mimetype", "application/pdf"],
            ["source", "drive.google.com"],
            ["lens", "Google Drive"],
          ],
          score: 1.0,
        },
      ],
    };
  } else if (func_name == "search_lenses") {
    return [
      {
        author: "a5huynh",
        description:
          "<div>Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.</div>",
        name: "fake_lense",
        label: "Fake Lense",
        hash: "",
        html_url: null,
        download_url: null,
        progress: { Finished: { num_docs: 100 } },
      },
      {
        author: "a5huynh",
        description:
          "<div>Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.</div>",
        name: "fake_lense_2_boogaloo",
        label: "Fake Lense 2: Boogaloo",
        hash: "",
        html_url: null,
        download_url: null,
        progress: { Finished: { num_docs: 100 } },
      },
    ];
  } else if (func_name == "list_connections") {
    return {
      supported: [
        {
          id: "api.github.com",
          label: "GitHub",
          description:
            "Search through your GitHub repositories, starred repositories, and follows",
        },
        {
          id: "calendar.google.com",
          label: "Google Calendar",
          description: "Adds indexing support for Google Calendar events.",
        },
        {
          id: "drive.google.com",
          label: "Google Drive",
          description: "Adds indexing support for Google Drive documents.",
        },
        {
          id: "api.examples.com",
          label: "Error Test",
          description:
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
        },
      ],
      user_connections: [
        {
          id: "calendar.google.com",
          account: "a5.t.huynh@gmail.com",
          is_syncing: false,
        },
        {
          id: "drive.google.com",
          account: "a5.t.huynh@gmail.com",
          is_syncing: true,
        },
        {
          id: "drive.google.com",
          account: "andrew@spyglass.fyi",
          is_syncing: false,
        },
      ],
    };
  } else if (func_name == "plugin:lens-updater|list_installed_lenses") {
    let list = [
      {
        author: "a5huynh",
        description:
          "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
        name: "stardew",
        label: "Stardew Valley",
        hash: "",
        html_url: null,
        download_url: null,
        lens_type: "Lens",
        categories: ["Games"],
        progress: {
          Installing: {
            percent: 45,
            status: "Downloading...",
          },
        },
      },
      {
        author: "a5huynh",
        description:
          "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
        name: "dnd",
        label: "Dungeons & Dragons",
        hash: "",
        html_url: null,
        download_url: null,
        lens_type: "Lens",
        categories: ["Games", "TTRPG"],
        progress: {
          Finished: {
            num_docs: 10000,
          },
        },
      },
      {
        author: "a5huynh",
        description:
          "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
        name: "2007scape",
        label: "Old School Runescape",
        hash: "",
        html_url: null,
        download_url: null,
        lens_type: "Lens",
        categories: ["Games", "MMORPG"],
        progress: {
          Installing: {
            percent: 45,
            status: "Crawling 10,123 of 20,454 (45%)",
          },
        },
      },
      {
        author: "Spyglass",
        description: "",
        name: "docs.google.com",
        label: "Google Docs",
        hash: "",
        html_url: null,
        download_url: null,
        lens_type: "Lens",
        categories: ["Productivity"],
        progress: {
          Installing: {
            percent: 100,
            status: "Crawled 12,334 of many",
          },
        },
      },
      {
        author: "Spyglass",
        description: "",
        name: "docs.google.com",
        label: "Google Calendar",
        hash: "",
        html_url: null,
        download_url: null,
        lens_type: "Lens",
        categories: ["Productivity"],
        progress: {
          Finished: {
            num_docs: 10000,
          },
        },
      },
      {
        author: "Spyglass",
        description: "",
        name: "docs.google.com",
        label: "Google Calendar",
        hash: "",
        html_url: null,
        download_url: null,
        lens_type: "Lens",
        categories: ["Productivity"],
        progress: {
          Finished: {
            num_docs: 10000,
          },
        },
      },
      {
        author: "Spyglass",
        description: "",
        name: "docs.google.com",
        label: "Google Calendar",
        hash: "",
        html_url: null,
        download_url: null,
        lens_type: "Lens",
        categories: ["Productivity"],
        progress: {
          Finished: {
            num_docs: 10000,
          },
        },
      },
      {
        author: "Spyglass",
        description: "",
        name: "docs.google.com",
        label: "Google Calendar",
        hash: "",
        html_url: null,
        download_url: null,
        lens_type: "Lens",
        categories: ["Productivity"],
        progress: {
          Finished: {
            num_docs: 10000,
          },
        },
      },
      {
        author: "Spyglass",
        description: "",
        name: "docs.google.com",
        label: "Google Calendar",
        hash: "",
        html_url: null,
        download_url: null,
        lens_type: "Lens",
        categories: ["Productivity"],
        progress: {
          Finished: {
            num_docs: 10000,
          },
        },
      },
    ];
    list.sort((a, b) => a.label.localeCompare(b.label));
    return list;
  } else if (func_name == "plugin:lens-updater|list_installable_lenses") {
    return [
      {
        author: "a5huynh",
        description:
          "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
        name: "installable",
        sha: "fake-sha",
        html_url: "https://example.com",
        download_url: "https://example.com",
        progress: "NotInstalled",
        categories: ["Test", "Games"],
      },
      {
        author: "a5huynh",
        description:
          "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
        name: "installable-two",
        sha: "fake-sha-1",
        html_url: "https://example.com",
        download_url: "https://example.com",
        progress: "NotInstalled",
        categories: ["Funk"],
      },
      {
        author: "billybob",
        description:
          "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
        name: "installable-three",
        sha: "fake-sha-1",
        html_url: "https://example.com",
        download_url: "https://example.com",
        progress: "NotInstalled",
        categories: ["Games", "TTRPG"],
      },
    ];
  } else if (func_name == "plugin:lens-updater|install_lens") {
    window.setTimeout(() => {
      CALLBACKS["RefreshDiscover"]();
    }, 5000);
  } else if (func_name == "plugin:lens-updater|uninstall_lens") {
    window.setTimeout(() => {
      CALLBACKS["RefreshLensLibrary"]();
    }, 5000);
  } else if (func_name == "list_plugins") {
    return [
      {
        author: "a5huynh",
        title: "chrome-exporter",
        description:
          "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
        is_enabled: true,
      },
      {
        author: "a5huynh",
        title: "local-file-indexer",
        description:
          "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam et vulputate urna, sit amet semper metus.",
        is_enabled: false,
      },
    ];
  } else if (func_name == "crawl_stats") {
    return {
      by_domain: [
        [
          "oldschool.runescape.wiki",
          {
            num_queued: 0,
            num_processing: 0,
            num_completed: 31413,
            num_indexed: 35453,
          },
        ],
        [
          "en.wikipedia.org",
          {
            num_queued: 0,
            num_processing: 0,
            num_completed: 31413,
            num_indexed: 35453,
          },
        ],
      ],
    };
  } else if (func_name == "load_user_settings") {
    return [
      [
        "_.data_directory",
        {
          label: "Data Directory",
          value:
            "/Users/a5huynh/Library/Application Support/com.athlabs.spyglass-dev",
          form_type: "Path",
          restart_required: true,
          help_text:
            "The data directory is where your index, lenses, plugins, and logs are stored. This will require a restart.",
        },
      ],
      [
        "_.autolaunch",
        {
          label: "Disable Autolaunch",
          value: "false",
          form_type: "Bool",
          restart_required: false,
          help_text:
            "Prevents Spyglass from automatically launching when your computer first starts up.",
        },
      ],
      [
        "_.disable_telemetry",
        {
          label: "Disable Telemetry",
          value: "false",
          form_type: "Bool",
          restart_required: false,
          help_text:
            "Stop sending data to any 3rd-party service. See https://spyglass.fyi/telemetry for more info.",
        },
      ],
    ];
  } else if (func_name == "plugin:tauri-plugin-startup|get_startup_progress") {
    return "Reticulating splines...";
  } else if (func_name == "authorize_connection") {
    if (params.id == "api.examples.com") {
      await new Promise((r) => setTimeout(r, 5000));
      throw "Unable to connect";
    } else {
      await new Promise((r) => setTimeout(r, 5000));
    }
    return [];
  } else if (func_name == "get_library_stats") {
    return {
      test_lens: {
        lens_name: "test_lens",
        crawled: 52358,
        enqueued: 1,
        indexed: 52357,
      },
    };
  } else if (func_name == "get_shortcut") {
    return "CmdOrCtrl+Shift+/";
  } else if (func_name == "default_indices") {
    return {
      file_paths: [
        "/Users/billy/Desktop",
        "/Users/billy/Documents",
        "/Applications",
      ],
    };
  } else if (func_name == "open_result") {
    window.open(params.url);
  }

  return [];
};

export let listen = async (event, callback) => {
  console.log(`listen called w/ ${event}`);
  CALLBACKS[event] = callback;
  return {};
};

export async function deleteDoc(id) {
  return await invoke("delete_doc", { id });
}

export async function network_change(isOffline) {
  return await invoke("network_change", { isOffline });
}

export async function recrawl_domain(domain) {
  return await invoke("recrawl_domain", { domain });
}

export async function save_user_settings(settings, restart) {
  return await invoke("save_user_settings", { settings, restart });
}

export async function searchDocs(lenses, query) {
  return await invoke("search_docs", { lenses, query });
}

export async function searchLenses(query) {
  return await invoke("search_lenses", { query });
}

export async function open_folder_path(path) {
  return await invoke("open_folder_path", { path });
}

export async function resizeWindow(height) {
  return await invoke("resize_window", { height });
}
