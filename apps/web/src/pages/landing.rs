use yew::prelude::*;

#[function_component(LandingPage)]
pub fn landing_page() -> Html {
    html! {
        <>
            <div class="p-16 text-center">
                <h1 class="text-5xl font-serif pb-4 px-8">
                    {"Conversational search for your "}
                    <span class="text-cyan-500">{"content"}</span>
                </h1>
                <div>
                    {"AI-powered search and answers. Across "}
                    <strong>{"all"}</strong>{" your content"}
                </div>
            </div>
            <div class="pt-16 border-t-2 border-neutral-900">
                <div class="px-16 md:px-8 grid grid-rows-3 md:grid-cols-3 gap-8 text-center align-top">
                    <div>
                        <div>{"Add your content"}</div>
                        <div class="text-sm text-neutral-400">
                            {"Add your website, podcasts, meetings notes. We support any content you
                            want to search & ask questions"}
                        </div>
                    </div>
                    <div>
                        <div>{"Customize"}</div>
                        <div class="text-sm text-neutral-400">
                            {"Give your search a name. Customize based on your brand. Make it
                            public and shareable with your friends!"}
                        </div>
                    </div>
                    <div>
                        <div>{"Start searching!"}</div>
                    </div>
                </div>
            </div>
        </>
    }
}