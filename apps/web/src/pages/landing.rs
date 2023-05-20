use ui_components::btn::{Btn, BtnSize, BtnType};
use yew::prelude::*;

#[function_component(LandingPage)]
pub fn landing_page() -> Html {
    html! {
        <>
            <div class="p-16 text-center">
                <h1 class="text-4xl md:text-6xl font-serif px-8">
                    {"Conversational search for your "}
                    <span class="text-cyan-500">{"content"}</span>
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
                    <a href="/lens/atp-podcast" class="block border border-neutral-600 p-4 rounded-md hover:border-cyan-500 cursor-pointer">
                        <div class="pb-2">{"💻 ATP: Accidental Tech Podcast"}</div>
                        <div class="text-sm text-neutral-400">
                            {"Search through the last 100 episodes of a podcast discussing tech, Apple, and programming."}
                        </div>
                        <div class="pt-4 text-xs">
                            <span class="text-neutral-400">{"source: "}</span>
                            <span class="underline text-cyan-500">{"atp.fm"}</span>
                        </div>
                    </a>

                    <a href="/lens/tim-ferris" class="border border-neutral-600 p-4 rounded-md hover:border-cyan-500 cursor-pointer">
                        <div class="pb-2">{"🎙️ The Tim Ferriss Show"}</div>
                        <div class="text-sm text-neutral-400">
                            {"Ask Tim Ferriss anything! Tim Ferriss is an American entrepreneur, investor, author, podcaster, and lifestyle guru."}
                        </div>
                        <div class="pt-4 text-xs">
                            <span class="text-neutral-400">{"source: "}</span>
                            <span class="underline text-cyan-500">{"tim.blog"}</span>
                        </div>
                    </a>

                    <a href="/lens/wine-folly" class="border border-neutral-600 p-4 rounded-md hover:border-cyan-500 cursor-pointer">
                        <div class="pb-2">{"🍷 Wine Folly"}</div>
                        <div class="text-sm text-neutral-400">
                            {"Want to learn more about wine? Search through the Wine Folly guides and
                            ask questions about your favorite wines."}
                        </div>
                        <div class="pt-4 text-xs">
                            <span class="text-neutral-400">{"source: "}</span>
                            <span class="underline text-cyan-500">{"winefolly.com"}</span>
                        </div>
                    </a>
                </div>
            </div>
        </>
    }
}
