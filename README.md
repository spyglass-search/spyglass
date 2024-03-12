<p align="center">
  <h1 align="center"><b>Spyglass</b></h1>
  <p align="center">
    A personal search engine that indexes what you want, exposing it to you in a simple & fast interface
    <br />
    <br />
        Download now:
        <a href="https://github.com/spyglass-search/spyglass/releases/download/v2023.4.1/Spyglass_23.4.1_universal.dmg">
            <strong>macOS (Intel/ARM)</strong>
        </a> |
        <a href="https://github.com/spyglass-search/spyglass/releases/download/v2023.4.1/Spyglass_23.4.1_x64_en-US.msi">
            <strong>Windows</strong>
        </a> |
        <a href="https://github.com/spyglass-search/spyglass/releases/download/v2023.4.1/spyglass_23.4.1_amd64.AppImage">
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
    <img src="https://github.com/spyglass-search/spyglass/actions/workflows/rust.yml/badge.svg">
    <a href="https://discord.gg/663wPVBSTB"><img src="https://img.shields.io/badge/Discord-Join%20Now-blue"></a>
  </p>
</p>

---

<p align="center">
    <br/>
    <img src="docs/spyglass-showcase.gif" style="border-radius: 8px">
</p>


## Create your library from:
- [x] Local documents/folders
- [x] Different internet topics (https://lenses.spyglass.fyi).
    - Lots of developer docs (Rustlang, Go, etc.)
    - Wikpedia, game wikis, etc.
- [x] Google Calendar events.
- [x] Google Drive docs.
- [x] GitHub repos, starred repos, & issues.
- [x] Reddit saved/upvoted posts.
- [ ] Gmail
- [ ] YouTube playlists & favorited.


## Introduction

Spyglass lives on your device crawling & indexing websites __you__ want with a basic
set of rules.

Web pages when condensed down to text are surprisingly small. With todays' incredibly
fast CPUs and ample amounts of of disk space, you can easily create a personal library of
wikis, blog posts, etc. that can be referenced instantly. Cut through the SEO spam of
the internet by building your own index.

For users who have been frustrated with the current state of search and the internet,
Spyglass offers a powerful solution to find _exactly_ what you want.

> See [Launching & Using Spyglass](https://docs.spyglass.fyi/usage/index.html) to get started.

## Traditional web search sucks

> The short answer is that Google search results are clearly dying. The long answer
> is that most of the web has become too inauthentic to trust.
>
> - https://dkb.io/post/google-search-is-dying

Spyglass is a solution to the following common issues when searching the web:

- Do you add terms such as `reddit` or `wiki` to your searches to narrow it down?
- Do you get frustrated with overzealous autocorrect on your search terms?
- Do you get frustrated with the terrible search some wikis/sites offer?
- Do you scroll past dozens of SEO spam pages to find the recipe/review/blog post you were looking for?
- Do you skip over a full-page of ads before getting to your actual search results?
- Do you have private websites / data / documents that you'd like to search through?

## How does it know what to crawl?

Spyglass expands on the ideas outlined in [this paper][googles-paper] by the
Brave Search Team. There are currently a simple set of rules that will point Spyglass
at a website and crawl only what you want. When available, crawling is
bootstrapped w/ data from the Internet Archive to not overwhelm smaller websites.

**For community lenses, we precrawl & preprocess these lenses so that you can get started
searching through those topics immediately.**

Not all websites & not all data can be crawled by Spyglass. If you have something
that you'd like to index and would like some help, feel free to ping me on
our [Discord server](https://discord.gg/663wPVBSTB)!

> See [Community Lenses](https://docs.spyglass.fyi/usage/lenses/community.html) to install
> lenses others in the community have built.

> See [Building your own lens](https://docs.spyglass.fyi/usage/lenses/build.html) to see
> how easy it is to build your own lens. Please share w/ the community when you're done!

[googles-paper]: https://brave.com/static-assets/files/goggles.pdf

## Developer Guide

If you'd like to help, reach out on our [Discord server](https://discord.gg/663wPVBSTB)
to see what is currently being developed and how you can help usher in a new,
better search.

> See [Building from source](https://docs.spyglass.fyi/build.html) to get started
> building & contributing to Spyglass.

TL;DR: If you want to build and run Spyglass from source, you can simply run this command:
```
cargo make run
```

