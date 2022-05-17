use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::crawl_stats;
use shared::response::{CrawlStats, QueueStatus};

fn fetch_crawl_stats(stats_handle: UseStateHandle<Vec<(String, QueueStatus)>>) {
    spawn_local(async move {
        match crawl_stats().await {
            Ok(results) => {
                let results: CrawlStats = results.into_serde().unwrap();
                let mut sorted = results.by_domain;
                sorted.sort_by(|(_, a), (_, b)| b.num_completed.cmp(&a.num_completed));
                stats_handle.set(sorted);
            }
            Err(e) => log::info!("Error: {:?}", e),
        }
    });
}

#[function_component(StatsPage)]
pub fn stats_page() -> Html {
    let stats: UseStateHandle<Vec<(String, QueueStatus)>> = use_state_eq(Vec::new);
    if stats.is_empty() {
        fetch_crawl_stats(stats.clone());
    }

    let onclick = {
        let stats = stats.clone();
        move |_| {
            stats.set(Vec::new());
            fetch_crawl_stats(stats.clone());
        }
    };

    let mut rendered = stats
        .iter()
        .map(|(domain, stats)| {
            let total = stats.total();

            let queued_per = stats.num_queued as f64 / total as f64 * 100.0;
            let processing_per = stats.num_processing as f64 / total as f64 * 100.0;
            let completed_per = stats.num_completed as f64 / total as f64 * 100.0;

            html! {
                <div class={"p-4 px-8"}>
                    <div class={"text-xs pb-1"}>
                        {domain}
                    </div>
                    <div class={"relative flex flex-row items-center flex-growgroup w-full"}>
                        <div class={"relative flex justify-center h-8 bg-neutral-600 p-2 rounded-l-lg"}
                            style={format!("width: {}%", queued_per)}>
                            <span class={"text-xs"}>{stats.num_queued}</span>
                        </div>
                        <div class={"relative flex justify-center h-8 bg-sky-600 p-2"}
                            style={format!("width: {}%", processing_per)}>
                            <span class={"text-xs"}>{stats.num_processing}</span>
                        </div>
                        <div class={"relative flex justify-center h-8 bg-lime-600 p-2 rounded-r-lg"}
                            style={format!("width: {}%", completed_per)}>
                            <span class={"text-xs"}>{stats.num_completed}</span>
                        </div>
                    </div>
                </div>
            }
        })
        .collect::<Html>();

    if stats.is_empty() {
        rendered = html! {
            <div class="flex justify-center">
                <div class="p-16">
                    <svg xmlns="http://www.w3.org/2000/svg" class="animate-spin h-16 w-16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                    </svg>
                </div>
            </div>
        }
    }

    html! {
        <div class={"text-white"}>
            <div class="pt-4 px-8 top-0 sticky bg-stone-900 z-40 h-32">
                <div class="flex flex-row items-center">
                    <h1 class={"text-2xl grow p-0"}>
                        {"Crawl Status"}
                    </h1>
                    <button
                        {onclick}
                        class="border border-neutral-600 rounded-lg p-2 active:bg-neutral-700 hover:bg-neutral-600">
                        <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                        </svg>
                    </button>
                </div>
                <div class="py-4">
                    <div class="flex flex-row">
                        <div class="flex flex-row items-center pb-2 text-xs mr-8">
                            <div class="relative flex w-4 h-4 bg-neutral-600 p-2 rounded-full mr-2"></div>
                            {"Queued"}
                        </div>
                        <div class="flex flex-row items-center pb-2 text-xs mr-8">
                            <div class="relative flex w-4 h-4 bg-sky-600 p-2 rounded-full mr-2"></div>
                            {"Processing"}
                        </div>
                        <div class="flex flex-row items-center pb-2 text-xs">
                            <div class="relative flex w-4 h-4 bg-lime-600 p-2 rounded-full mr-2"></div>
                            {"Completed"}
                        </div>
                    </div>
                </div>
            </div>
            <div class={"divide-y divide-neutral-600"}>
                {rendered}
            </div>
        </div>
    }
}
