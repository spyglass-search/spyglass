use num_format::{Buffer, Locale};
use shared::event::ClientInvoke;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::{btn, icons};
use crate::invoke;
use shared::response::{CrawlStats, QueueStatus};

fn fetch_crawl_stats(
    stats_handle: UseStateHandle<Vec<(String, QueueStatus)>>,
    request_finished: UseStateHandle<bool>,
) {
    spawn_local(async move {
        match invoke(ClientInvoke::GetCrawlStats.as_ref(), JsValue::NULL).await {
            Ok(results) => {
                let results: CrawlStats = results.into_serde().unwrap();
                let mut sorted = results.by_domain;
                sorted.sort_by(|(_, a), (_, b)| b.num_completed.cmp(&a.num_completed));
                stats_handle.set(sorted);
                request_finished.set(true);
            }
            Err(e) => {
                log::info!("Error: {:?}", e);
                request_finished.set(true);
            }
        }
    });
}

#[derive(Properties, PartialEq, Eq)]
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
        "mr-2".into(),
        "rounded-full".into(),
        props.color.clone(),
    ];

    html! {
        <div class="flex flex-row items-center pb-2 text-xs mr-8">
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
    let percent = (props.count as f64 / props.total * 100.0).max(5.0);
    let mut buf = Buffer::default();
    buf.write_formatted(&props.count, &Locale::en);

    let bar_style = classes!(
        "relative",
        "flex",
        "justify-center",
        "h-8",
        "p-2",
        props.color.clone(),
        props.is_start.then_some(Some("rounded-l-lg")),
        props.is_end.then_some(Some("rounded-r-lg")),
    );

    html! {
        <div class={bar_style} style={format!("width: {}%", percent)}>
            <span class="text-xs">{buf.as_str()}</span>
        </div>
    }
}

#[function_component(StatsPage)]
pub fn stats_page() -> Html {
    let stats: UseStateHandle<Vec<(String, QueueStatus)>> = use_state_eq(Vec::new);
    let request_finished = use_state(|| false);

    if stats.is_empty() && !(*request_finished) {
        fetch_crawl_stats(stats.clone(), request_finished.clone());
    }

    let onclick = {
        let request_finished = request_finished.clone();
        let stats = stats.clone();
        Callback::from(move |_| {
            request_finished.set(false);
            stats.set(Vec::new());
            fetch_crawl_stats(stats.clone(), request_finished.clone());
        })
    };

    let mut rendered = stats
        .iter()
        .map(|(domain, stats)| {
            let total = stats.total() as f64;
            html! {
                <div class="p-4 px-8">
                    <div class="text-xs pb-2 flex flex-row gap-2">
                        <div class="flex-grow">{domain}</div>
                        <btn::RecrawlButton onrecrawl={onclick.clone()} domain={domain.clone()} />
                        <btn::DeleteDomainButton ondelete={onclick.clone()} domain={domain.clone()} />
                    </div>
                    <div class="relative flex flex-row items-center flex-growgroup w-full">
                        <StatsBar count={stats.num_queued} total={total} color={"bg-neutral-600"} is_start={true} />
                        <StatsBar count={stats.num_processing} total={total} color={"bg-sky-600"} />
                        <StatsBar count={stats.num_completed} total={total} color={"bg-lime-600"} />
                        <StatsBar count={stats.num_indexed} total={total} color={"bg-lime-800"} is_end={true} />
                    </div>
                </div>
            }
        })
        .collect::<Html>();

    if stats.is_empty() && !(*request_finished) {
        rendered = html! {
            <div class="flex justify-center">
                <div class="p-16">
                    <icons::RefreshIcon height={"h-16"} width={"w-16"} animate_spin={true} />
                </div>
            </div>
        }
    } else if stats.is_empty() && *request_finished {
        rendered = html! {
            <div class="flex justify-center">
                <div class="block p-16 text-center">
                    <icons::EmojiSadIcon height={"h-16"} width={"w-16"} />
                </div>
            </div>
        }
    }

    html! {
        <div class="text-white bg-neutral-800 h-full">
            <div class="py-4 px-8 top-0 sticky bg-stone-800 z-40 border-b-2 border-stone-900">
                <div class="flex flex-row items-center">
                    <h1 class="text-2xl grow p-0">
                        {"Crawl Status"}
                    </h1>
                    <btn::Btn onclick={onclick}>
                        <icons::RefreshIcon height={"h-4"} width={"w-4"} />
                    </btn::Btn>
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
            <div class="divide-y divide-neutral-600">
                {rendered}
            </div>
        </div>
    }
}
