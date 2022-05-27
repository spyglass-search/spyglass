use yew::function_component;
use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::components::ResultListData;

#[derive(Properties, PartialEq)]
pub struct LensProps {
    pub result: ResultListData,
}

#[function_component(Lens)]
pub fn lens_component(props: &LensProps) -> Html {
    let component_styles: Vec<String> = vec![
        "border-t".into(),
        "border-neutral-600".into(),
        "p-4".into(),
        "pr-0".into(),
        "text-white".into(),
        "bg-netural-800".into(),
    ];

    let result = &props.result;
    html! {
        <div class={component_styles}>
            <h2 class="text-xl truncate p-0">
                {result.title.clone()}
            </h2>
            <h2 class="text-sm truncate py-1 text-neutral-400">
                {"Crafted By:"}
                <a class="ml-2 text-cyan-400">{"@a5huynh"}</a>
            </h2>
            <div class="text-sm leading-relaxed text-neutral-400 h-6 overflow-hidden text-ellipsis">
                {result.description.clone()}
            </div>
            <div class="pt-2 flex flex-row gap-8">
                <a class="flex flex-row text-cyan-400 text-sm">
                    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                        <path fill-rule="evenodd" d="M6 2a2 2 0 00-2 2v12a2 2 0 002 2h8a2 2 0 002-2V7.414A2 2 0 0015.414 6L12 2.586A2 2 0 0010.586 2H6zm5 6a1 1 0 10-2 0v3.586l-1.293-1.293a1 1 0 10-1.414 1.414l3 3a1 1 0 001.414 0l3-3a1 1 0 00-1.414-1.414L11 11.586V8z" clip-rule="evenodd" />
                    </svg>
                    <div class="ml-2">{"Install"}</div>
                </a>

                <a class="flex flex-row text-neutral-400 text-sm">
                    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                        <path d="M10 12a2 2 0 100-4 2 2 0 000 4z" />
                        <path fill-rule="evenodd" d="M.458 10C1.732 5.943 5.522 3 10 3s8.268 2.943 9.542 7c-1.274 4.057-5.064 7-9.542 7S1.732 14.057.458 10zM14 10a4 4 0 11-8 0 4 4 0 018 0z" clip-rule="evenodd" />
                    </svg>
                    <div class="ml-2">{"View Source"}</div>
                </a>
            </div>
        </div>
    }
}

#[function_component(LensManagerPage)]
pub fn lens_manager_page() -> Html {
    let lenses: UseStateHandle<Vec<ResultListData>> = use_state_eq(Vec::new);
    let _request_finished = use_state(|| false);

    let on_open_folder = { move |_| {
        spawn_local(async {
            let _ = crate::open_lens_folder().await;
        });
    } };

    html! {
        <div class="text-white">
            <div class="pt-4 px-8 top-0 sticky bg-stone-900 z-400 h-20">
                <div class="flex flex-row items-center gap-4">
                    <h1 class="text-2xl grow">{"Lens Manager"}</h1>
                    <button
                        onclick={on_open_folder}
                        class="flex flex-row border border-neutral-600 rounded-lg p-2 active:bg-neutral-700 hover:bg-neutral-600 text-sm">
                        <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                            <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1H8a3 3 0 00-3 3v1.5a1.5 1.5 0 01-3 0V6z" clip-rule="evenodd" />
                            <path d="M6 12a2 2 0 012-2h8a2 2 0 012 2v2a2 2 0 01-2 2H2h2a2 2 0 002-2v-2z" />
                        </svg>
                        <div class="ml-2">{"Lens folder"}</div>
                    </button>
                    <button
                        class="border border-neutral-600 rounded-lg p-2 active:bg-neutral-700 hover:bg-neutral-600">
                        <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                        </svg>
                    </button>
                </div>
            </div>
            <div class="px-8">
                {
                    lenses.iter().map(|data| {
                        html! {<Lens result={data.clone()} /> }
                    }).collect::<Html>()
                }
            </div>
        </div>
    }
}
