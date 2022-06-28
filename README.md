<p align="center">
  <h1 align="center"><b>Spyglass</b></h1>
  <p align="center">
    A personal search engine that indexes what you want, exposing it to you in a simple & fast interface
    <br />
    <br />
        Download now:
        <a href="https://github.com/a5huynh/spyglass/releases/download/v2022.6.4/Spyglass_22.6.4_x64.dmg">
            <strong>macOS</strong>
        </a> |
        <a href="https://github.com/a5huynh/spyglass/releases/download/v2022.6.4/Spyglass_22.6.4_x64_en-US.msi">
            <strong>Windows</strong>
        </a> |
        <a href="https://github.com/a5huynh/spyglass/releases/download/v2022.6.4/spyglass_22.6.4_amd64.AppImage">
            <strong>Linux (AppImage)</strong>
        </a>
    <br />
    <br />
    <a href="https://docs.spyglass.fyi">
        <strong>Documentation</strong>
    </a> |
    <a href="https://docs.spyglass.fyi/usage/index.html">
        <strong>Using Spyglass</strong>
    </a> |
    <a href="https://docs.spyglass.fyi/usage/lenses/index.html">
        <strong>Lenses</strong>
    </a>
    <br />
    <br />
    <img src="https://github.com/a5huynh/spyglass/actions/workflows/rust.yml/badge.svg">
    <a href="https://discord.gg/663wPVBSTB"><img src="https://img.shields.io/badge/Discord-join%20the%20community-blue"></a>
  </p>
</p>

---

<p align="center">
    <br/>
    <img src="./docs/spyglass-poc.gif">
</p>

Spyglass is an open-source, cross-platform search engine that lives on your machine,
indexing what you want, and provides a fast & simple way to access your data.

## Why Spyglass?

Spyglass is a solution to address the following common issues when searching the web:

* Do you add terms such as `reddit` or `wiki` to your searches to narrow it down?
* Do you skip over a full-page of ads before getting to your actual search results
* Do you scroll past dozens of SEO spam pages to find the recipe/review/blog post you were looking for?
* Do you get frustrated with overzealous autocorrect on your search terms?

> See [Using Spyglass](https://docs.spyglass.fyi/usage/index.html) to get started.

## How does it know what to crawl?

Spyglass expands on the ideas outlined in [this paper][googles-paper] by the
Brave Search Team.

You can add different lenses that clue the application into what you want to have indexed.
Click on "Manage/install lenses" from the menubar icon to open up the "Lens Manager" as
seen below. From here, you can one-click install lenses from our community and the crawler
will happily go out and start indexing.

> See [Community Lenses](https://docs.spyglass.fyi/usage/lenses/community.html) to install
lenses others in the community have built.

> See [Building your own lens](https://docs.spyglass.fyi/usage/lenses/build.html) to see
how easy it is to build your own lens.

[googles-paper]: https://brave.com/static-assets/files/goggles.pdf

## Developer Contribution