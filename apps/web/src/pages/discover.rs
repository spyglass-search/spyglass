use ui_components::btn::{Btn, BtnSize, BtnType};
use ui_components::icons;
use yew::prelude::*;

use crate::pages::landing::PublicExample;

#[derive(Properties, PartialEq)]
pub struct DiscoverPageProps;

#[function_component(DiscoverPage)]
pub fn discover_page(_: &DiscoverPageProps) -> Html {
    html! {
        <div class="flex flex-col gap-8 p-8">
            <div class="text-center">
                <h1 class="text-4xl font-serif">
                    {"Try it out!"}
                </h1>
                <div class="text-neutral-400 text-xl">
                    {"Search, ask questions, and explore our featured communities."}
                </div>
                <div class="pt-4 text-base flex flex-row place-content-center items-center gap-4">
                    <div>{"Want to add your favorite community?"}</div>
                    <Btn
                        href="https://twitter.com/intent/tweet?text=Hey%20%40a5huynh%20%40spyglassfyi%2C%20can%20you%20add%20...%3F"
                        size={BtnSize::Sm}
                        _type={BtnType::Primary}
                    >
                        <icons::Twitter height="h-4" width="w-4" />
                        {"Request on Twitter"}
                    </Btn>
                    <Btn
                        href="https://mastodon.social/share?text=Hey%20@a5huynh%20can%20you%20add%20...%3F"
                        size={BtnSize::Sm}
                        _type={BtnType::Primary}
                    >
                        <icons::Mastodon height="h-4" width="w-4" />
                        {"Request on Mastodon"}
                    </Btn>
                </div>
            </div>

            <div>
                <h1 class="text-2xl py-4">
                    {"Documentation"}
                </h1>
                <div class="grid grid-cols-1 gap-4 md:grid-cols-3">
                    <PublicExample
                        href="/lens/adobe-experience-league"
                        name="Adobe Experience League"
                        description="Adobe Experience League is a vast library of learning content and courses for Adobe Enterprise products."
                        sources={vec!["experience.adobe.com".into()]}
                    />

                    <PublicExample
                        href="/lens/ansible"
                        name="Ansible"
                        description="Ansible is a suite of software tools that enables infrastructure as code. It is open-source and the suite includes software provisioning, configuration management, and application deployment functionality."
                        sources={vec!["ansible.com".into()]}
                    />

                    <PublicExample
                        href="/lens/bootstrap"
                        name="Bootstrap"
                        description="Bootstrap is a powerful, feature-packed frontend toolkit. Build anything—from prototype to production—in minutes."
                        sources={vec!["getbootstrap.com".into()]}
                    />

                    <PublicExample
                        href="/lens/css"
                        name="CSS"
                        description="Cascading Style Sheets (CSS) is a stylesheet language used to describe the presentation of a document written in HTML."
                        sources={vec!["developer.mozilla.org".into()]}
                    />

                    <PublicExample
                        href="/lens/golang"
                        name="Go Programming Language"
                        description="Go is a statically typed, compiled high-level programming language designed at Google by Robert Griesemer, Rob Pike, and Ken Thompson."
                        sources={vec!["go.dev".into()]}
                    />

                    <PublicExample
                        href="/lens/html"
                        name="HTML"
                        description="HTML (HyperText Markup Language) is the most basic building block of the Web. It defines the meaning and structure of web content."
                        sources={vec!["developer.mozilla.org".into()]}
                    />

                    <PublicExample
                        href="/lens/java_se19"
                        name="Java Programming Language"
                        description="Java is a high-level, class-based, object-oriented programming language that is designed to have as few implementation dependencies as possible."
                        sources={vec!["docs.oracle.com/en/java".into()]}
                    />

                    <PublicExample
                        href="/lens/rustlang"
                        name="Rust Programming Language"
                        description="Rust is a multi-paradigm, general-purpose programming language that emphasizes performance, type safety, and concurrency."
                        sources={vec!["doc.rust-lang.org".into(), "docs.rs".into()]}
                    />

                    <PublicExample
                        href="/lens/nix-docs"
                        name="NixOS documentation"
                        description="Nix is a powerful package manager for Linux and other Unix systems that makes package management reliable and reproducible."
                        sources={vec![
                            "nixos.org".into(),
                            "nix.dev".into(),
                            "nixos.wiki".into(),
                            "any many more...".into()
                        ]}
                    />

                </div>
            </div>

            <div>
                <h1 class="text-2xl py-4">
                    {"Podcasts"}
                </h1>
                <div class="grid grid-cols-1 gap-4 md:grid-cols-3">
                    <PublicExample
                        href="/lens/atp-podcast"
                        name="💻 ATP: Accidental Tech Podcast"
                        description="Search through the last 100 episodes of a podcast discussing tech, Apple, and programming."
                        sources={vec!["atp.fm".into()]}
                    />
                    <PublicExample
                        href="/lens/mac-power-users"
                        name="🍎 Mac Power Users"
                        description="Learn about getting the most from your Apple technology with focused topics and workflow guests. Creating Mac Power Users, one geek at a time since 2009."
                        sources={vec!["relay.fm/mpu".into()]}
                    />

                    <PublicExample
                        href="/lens/20minutevc"
                        name="⏳ The 20 Minute VC"
                        description="The Twenty Minute VC (20VC) interviews the world's greatest venture capitalists with prior guests including Sequoia's Doug Leone and Benchmark's Bill Gurley."
                        sources={vec!["thetwentyminutevc.com".into()]}
                    />

                    <PublicExample
                        href="/lens/tim-ferris"
                        name="🎙️ The Tim Ferriss Show"
                        description="Ask Tim Ferriss anything! Tim Ferriss is an American entrepreneur, investor, author, podcaster, and lifestyle guru."
                        sources={vec!["tim.blog".into()]}
                    />
                </div>
            </div>

            <div>
                <h1 class="text-2xl py-4">
                    {"Just for Fun! 🍷 🐉"}
                </h1>
                <div class="grid grid-cols-1 gap-4 md:grid-cols-3">
                    <PublicExample
                        href="/lens/wine-folly"
                        name="🍷 Wine Folly"
                        description="Want to learn more about wine? Search through the Wine Folly guides and ask questions about your favorite wines."
                        sources={vec!["winefolly.com".into()]}
                    />

                    <PublicExample
                        href="/lens/dnd"
                        name="⚔️🐉 Dungeons & Dragons"
                        description="Unsure about a rule? Search and ask questions about D&D 5E items, rules, monsters, and more."
                        sources={vec!["dndbeyond.fm".into(),"roll20.net".into()]}
                    />
                </div>
            </div>
        </div>
    }
}
