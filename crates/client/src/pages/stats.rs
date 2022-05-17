use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::crawl_stats;
use shared::response::{CrawlStats, QueueStatus};

#[function_component(StatsPage)]
pub fn stats_page() -> Html {
    let stats: UseStateHandle<Vec<(String, QueueStatus)>> = use_state_eq(Vec::new);

    {
        let stats = stats.clone();
        use_effect(move || {
            spawn_local(async move {
                match crawl_stats().await {
                    Ok(results) => {
                        let results: CrawlStats = results.into_serde().unwrap();
                        let mut sorted = results.by_domain;
                        sorted.sort_by(|(_, a), (_, b)| b.total().cmp(&a.total()));
                        stats.set(sorted);
                    }
                    Err(e) => log::info!("Error: {:?}", e),
                }
            });
            || ()
        })
    }

    let rendered = stats
        .iter()
        .map(|(domain, stats)| {
            let total = stats.total();

            let queued_per = stats.num_queued as f64 / total as f64 * 100.0;
            let processing_per = stats.num_processing as f64 / total as f64 * 100.0;
            let completed_per = stats.num_completed as f64 / total as f64 * 100.0;

            html! {
                <div class={"py-4"}>
                    <div class={"text-xs pb-1"}>
                        {domain}
                    </div>
                    <div class={"relative flex flex-row items-center flex-growgroup w-full"}>
                        <div class={"relative flex justify-center h-8 bg-indigo-400 p-2"}
                            style={format!("width: {}%", queued_per)}>
                            <span class={"text-xs"}>{stats.num_queued}</span>
                        </div>
                        <div class={"relative flex justify-center h-8 bg-indigo-500 p-2"}
                            style={format!("width: {}%", processing_per)}>
                            <span class={"text-xs"}>{stats.num_processing}</span>
                        </div>
                        <div class={"relative flex justify-center h-8 bg-indigo-600 p-2"}
                            style={format!("width: {}%", completed_per)}>
                            <span class={"text-xs"}>{stats.num_completed}</span>
                        </div>
                    </div>
                </div>
            }
        })
        .collect::<Html>();

    html! {
        <div class={"text-white p-4"}>
            <h1 class={"text-2xl"}>
                {"Crawl Status"}
            </h1>
            <div class={"py-4"}>
                <div class={"relative w-full grid grid-cols-3 gap-2"}>
                    <div class={"flex flex-row justify-center items-center pb-2 text-xs"}>
                        <div class="relative flex w-4 h-4 bg-indigo-400 p-2 mr-2"></div>
                        {"Queued"}
                    </div>
                    <div class={"flex flex-row justify-center items-center pb-2 text-xs"}>
                        <div class="relative flex w-4 h-4 bg-indigo-500 p-2 mr-2"></div>
                        {"Processing"}
                    </div>
                    <div class={"flex flex-row justify-center items-center pb-2 text-xs"}>
                        <div class="relative flex w-4 h-4 bg-indigo-600 p-2 mr-2"></div>
                        {"Completed"}
                    </div>
                </div>
            </div>
            <div class={"divide-y divide-neutral-600"}>
                {rendered}
            </div>
        </div>
    }
}
