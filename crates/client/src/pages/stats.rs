use num_format::{Buffer, Locale};
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

#[derive(Properties, PartialEq)]
pub struct LegendIconProps {
    pub color: String,
    pub label: String,
}

#[function_component(LegendIcon)]
pub fn legend_icon(props: &LegendIconProps) -> Html {
    let legend_styles: Vec<String> = vec![
        "relative".into(),
        "flex".into(),
        "w-4".into(),
        "h-4".into(),
        "p-2".into(),
        "rounded-full".into(),
        props.color.clone(),
    ];

    html! {
        <div class={"flex flex-row items-center pb-2 text-xs mr-8"}>
            <div class={legend_styles}></div>
            {props.label.clone()}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct StatsBarProps {
    count: u64,
    total: f64,
    color: String,
    #[prop_or_default]
    is_start: bool,
    #[prop_or_default]
    is_end: bool,
}

#[function_component(StatsBar)]
fn stats_bar(props: &StatsBarProps) -> Html {
    let percent = props.count as f64 / props.total * 100.0;
    let mut buf = Buffer::default();
    buf.write_formatted(&props.count, &Locale::en);

    let mut bar_style: Vec<String> = vec![
        "relative".into(),
        "flex".into(),
        "justify-center".into(),
        "h-8".into(),
        "p-2".into(),
    ];
    bar_style.push(props.color.clone());

    if props.is_start {
        bar_style.push("rounded-l-lg".into());
    }

    if props.is_end {
        bar_style.push("rounded-r-lg".into());
    }

    html! {
        <div class={bar_style} style={format!("width: {}%", percent)}>
            <span class={"text-xs"}>{buf.as_str()}</span>
        </div>
    }
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
            let total = stats.total() as f64;
            html! {
                <div class={"p-4 px-8"}>
                    <div class={"text-xs pb-1"}>
                        {domain}
                    </div>
                    <div class={"relative flex flex-row items-center flex-growgroup w-full"}>
                        <StatsBar count={stats.num_queued} total={total} color={"bg-neutral-600"} is_start={true} />
                        <StatsBar count={stats.num_processing} total={total} color={"bg-sky-600"} />
                        <StatsBar count={stats.num_completed} total={total} color={"bg-lime-600"} />
                        <StatsBar count={stats.num_indexed} total={total} color={"bg-lime-800"} is_end={true} />
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
            <div class="pt-4 px-8 top-0 sticky bg-stone-900 z-40 h-24">
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
                <div class="py-2">
                    <div class="flex flex-row">
                        <LegendIcon label="Queued" color="bg-neutral-600" />
                        <LegendIcon label="Processing" color="bg-sky-600" />
                        <LegendIcon label="Completed" color="bg-lime-600" />
                        <LegendIcon label="Indexed" color="bg-lime-800" />
                    </div>
                </div>
            </div>
            <div class={"divide-y divide-neutral-600"}>
                {rendered}
            </div>
        </div>
    }
}
