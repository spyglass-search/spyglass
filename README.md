![check/build workflow](https://github.com/a5huynh/spyglass/actions/workflows/rust.yml/badge.svg)
[![](https://img.shields.io/badge/discord-join%20the%20community-blue)](https://discord.gg/663wPVBSTB)

# Spyglass

## tl; dr; Spyglass indexes what you want exposing it to you in a super simple & fast interface

⚠️ Spyglass is very much in its early stages, but it’s in a place where it's functional and can be used to replace basic searches. ⚠️

Download now: [Mac](https://github.com/a5huynh/spyglass/releases/download/v2022.5.15/Spyglass_22.5.15_x64.dmg) | [Windows](https://github.com/a5huynh/spyglass/releases/download/v2022.5.15/Spyglass_22.5.15_x64_en-US.msi) | [Linux (AppImage)](https://github.com/a5huynh/spyglass/releases/download/v2022.5.15/spyglass_22.5.15_amd64.AppImage)

---

## Table of Contents

* [Installation](#installation)
* [Spyglass in Action](#spyglass-in-action)
* [Why Spyglass](#why-spyglass)
* [How does it know what to crawl](#how-does-it-know-what-to-crawl)
  * [Example: Curated recipe searching](#curated-recipe-searching)
  * [Example: Narrowing down by a specific topic](#curated-recipe-searching)
* [Settings](#settings)
  * [Updating the shorcut](#updating-the-shortcut)

---

## Installation

Stable compiled builds are provided on the [releases](https://github.com/a5huynh/spyglass/releases) pages.
Download the appriopriate file for your OS (e.g. `.deb` for linux, `.dmg` for macOS and `.msi` for Windows)

If you're interested in building from source, after checking out the repository run the following:

```
make setup-dev
make build-release
```

## Spyglass in action

Once launched, press **`Cmd + Shift + /`** to open Spyglass. If the app has been
successfully launched, you'll see a little menubar icon like the following:

![Menubar icon and menu](docs/menubar-menu.png)


Queries prefixed with `/` will search through your installed lenses, otherwise it'll
search through your index. Use the arrow keys to select the result you want and hit
`Enter` to open the link in the browser of your choice!

[![Spyglass in action!](docs/spyglass-poc.gif)](https://www.youtube.com/embed/OzNrxtM3s_8)


## Why Spyglass?

Spyglass is a solution to address the following common issues when searching the web.
* Do you add terms such as `reddit` or `wiki` to your searches to narrow it down?
* Do you skip over a full-page of ads before getting to your actual search results
* Do you scroll past dozens of SEO spam pages to find the recipe/review/blog post you were looking for?
* Do you get frustrated with overzealous autocorrect on your search terms?


## How does it know what to crawl?

Spyglass expands on the ideas outlined in [this paper][googles-paper] by the
Brave Search Team.

[googles-paper]: https://brave.com/static-assets/files/goggles.pdf

You can add different lenses that clue the application into what you want to have indexed.
Here are some examples that I've been personally using:


### Curated recipe searching

Interested in cooking & recipes? Add a "recipe" lens which will go index a
curated set of websites with high quality recipes.

``` rust
(
    version: "1",
    // Be proud of your creation :). Maybe soon we can share these ;)
    author: "Andrew Huynh",
    name: "recipes",
    description: Some(r#"
        A curated collection of websites with useful, high-quality recipes.
    "#),
    domains: [

        // Major sites that often have really good recipes
        "www.seriouseats.com",
        "cooking.nytimes.com",
        ...

        // Specific cuisines/sites that I've found randomly w/ high-quality recipes
        "www.hungryhuy.com",
        "www.vickypham.com",
    ],

    urls: [
        // URLs are considered prefixed, i.e. anything that starts w/ the following
        // will be matched and crawled.
        //
        // https://www.reddit.com/r/recipes/ -> matches
        // https://www.reddit.com/r/recipes_not/ -> does not matche, notice the end slash.
        "https://www.reddit.com/r/recipes/",
    ]
)
```


### Narrowing down by a specific topic

Interested in the Rust programming language? Add the "rustlang" lens which will
index the Rust book, rust docs, crate.io, and other sites that are related to the
programming language and not the Rust game / The Rust Belt / oxidation / etc.

``` rust
(
    version: "1",
    author: "Andrew Huynh",
    name: "rustlang",
    description: Some("Rustlang targeted websites"),
    domains: [
        // Support for wildcards in domain names
        "*.rust-lang.org",
        "docs.rs",
        "rustconf.com",
        "crates.io",
        "this-week-in-rust.org",
        ...
    ],

    urls: [
        "https://www.reddit.com/r/rust/",
        "https://www.reddit.com/r/rust_gamedev/",
    ]
)
```


## Settings

The `settings.ron` file can be found by "Show Settings folder". If there is no
file found in their directory on startup, a default one will be created.

``` rust
(
    // The max number of pages to index per domain
    domain_crawl_limit: Finite(1000),
    // The max number of crawlers per domain
    inflight_domain_limit: Finite(2),
    // The max number of crawlers in total
    inflight_crawl_limit: Finite(10),
    // Not used... yet!
    run_wizard: false,
    // Not used... yet!
    allow_list: [],
    // Domains to completely ignore, regardless of the lenses you have installed.
    block_list: [
      "web.archive.org",
      "w3schools.com"
    ],
    // Shortcut to launch the search bar
    shortcut: "CmdOrCtrl+Shift+/",
    // Where to store your index and index metadata
    // The exact default location is dependent on your OS
    data_directory: "/Users/<username>/Library/Application Support/com.athlabs.spyglass",
    // By default, Spyglass will only crawl things as specified in your lenses. If you want
    // to follow links without regard to those rules, set this to true.
    crawl_external_links: false,
)
```

### Updating the Shortcut

To update the shortcut combine the following modifiers w/ an appropriate
[keycode](https://docs.rs/tao/0.8.3/tao/keyboard/enum.KeyCode.html) combining each key with a "+".

Supported Modifiers:

* "Option" / "Alt"
* "Control" / "Ctrl"
* "Command" / "Cmd" / "Super"
* "Shift"
* "CmdOrCtrl"

Examples:

* "CmdOrCtrl+/" => Launches the app w/ `Cmd` or `Ctrl` + `/`
* "CmdOrCtrl+Shift+/" => Launches the app w/ `Cmd` or `Ctrl` + `Shift` + `/`
* "Shift+4" => Launches the app w/ `Shift` + `4`

NOTE: Shortcuts are allowed to have any number of modifiers but only a *single* key.
For example, `Shift+4` will work but not `Shift+4+2`
