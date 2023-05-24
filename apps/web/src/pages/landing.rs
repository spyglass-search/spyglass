use ui_components::btn::{Btn, BtnSize, BtnType};
use yew::{platform::spawn_local, prelude::*};

use crate::{
    auth0_login,
    metrics::{Metrics, WebClientEvent},
};

#[derive(Properties, PartialEq)]
pub struct LandingPageProps {
    pub session_uuid: String,
}

#[function_component(LandingPage)]
pub fn landing_page(props: &LandingPageProps) -> Html {
    let metrics = Metrics::new(false);
    let uuid = props.session_uuid.clone();
    let auth_login = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        let metrics = metrics.clone();
        let uuid = uuid.clone();
        spawn_local(async move {
            metrics.track(WebClientEvent::Login, &uuid).await;
            let _ = auth0_login().await;
        });
    });

    html! {
        <>
            <div class="p-16 text-center">
                <h1 class="text-4xl md:text-6xl font-serif px-8">
                    {"Conversational search for your "}
                    <span class="text-cyan-500">{"community"}</span>
                    {"."}
                </h1>
                <div class="text-neutral-400 text-xl">
                    {"AI-powered "}
                    <span class="text-white font-bold">{"search"}</span>
                    {" and "}
                    <span class="text-white font-bold">{"answers."}</span>
                    {" Across"}
                    <span class="text-white font-bold">{" all "}</span>
                    {"your content"}
                </div>
                <div class="mt-8 text-center w-fit mx-auto">
                    <Btn href="https://airtable.com/shrEW2xhITj3zf7sw"
                        _type={BtnType::Primary}
                        size={BtnSize::Xl}
                        classes={"inline-block"}
                    >
                        {"Join our waitlist"}
                    </Btn>
                    <div class="pt-2 text-sm">
                        {"Already have an account?"}
                        <a class="text-cyan-500 ml-2 font-semibold cursor-pointer" onclick={auth_login}>
                            {"Sign in"}
                        </a>
                    </div>
                </div>
            </div>
            <div class="pt-8">
                <div class="text-center pb-4 px-8">
                    <h1 class="text-4xl font-serif px-8">
                        {"Try it out on our public datasets"}
                    </h1>
                    <div class="text-neutral-400 text-xl">
                        {"Search, ask questions, and explore new topics in a completely new way."}
                    </div>
                </div>
                <div class="grid grid-rows-3 gap-4 px-8 md:px-16 align-top md:grid-cols-3">
                    <PublicExample
                        href="/lens/atp-podcast"
                        name="💻 ATP: Accidental Tech Podcast"
                        description="Search through the last 100 episodes of a podcast discussing tech, Apple, and programming."
                        sources={vec!["atp.fm".into()]}
                    />

                    <PublicExample
                        href="/lens/dnd"
                        name="⚔️🐉 Dungeons & Dragons"
                        description="Unsure about a rule? Search and ask questions about D&D 5E items, rules, monsters, and more."
                        sources={vec!["dndbeyond.fm".into(),"roll20.net".into()]}
                    />

                    <PublicExample
                        href="/lens/tim-ferris"
                        name="🎙️ The Tim Ferriss Show"
                        description="Ask Tim Ferriss anything! Tim Ferriss is an American entrepreneur, investor, author, podcaster, and lifestyle guru."
                        sources={vec!["tim.blog".into()]}
                    />

                    <PublicExample
                        href="/lens/wine-folly"
                        name="🍷 Wine Folly"
                        description="Want to learn more about wine? Search through the Wine Folly guides and ask questions about your favorite wines."
                        sources={vec!["winefolly.com".into()]}
                    />
                </div>
            </div>
        </>
    }
}

#[derive(Properties, PartialEq)]
struct PublicExampleProps {
    href: String,
    name: String,
    description: String,
    sources: Vec<String>,
}

#[function_component(PublicExample)]
fn pub_example(props: &PublicExampleProps) -> Html {
    let sources = props.sources.iter()
        .map(|source| html! {
            <span class="ml-2 underline text-cyan-500">{source}</span>
        })
        .collect::<Html>();

    html! {
        <a
            href={props.href.clone()}
            class="flex flex-col justify-between border border-neutral-600 p-4 rounded-md hover:border-cyan-500 cursor-pointer"
        >
            <div class="pb-2">{props.name.clone()}</div>
            <div class="text-sm text-neutral-400">{props.description.clone()}</div>
            <div class="pt-4 text-xs mt-auto">
                <span class="text-neutral-400">{"source:"}</span>
                {sources}
            </div>
        </a>
    }
}
