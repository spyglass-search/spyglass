use crate::components::forms::SettingChangeEvent;
use crate::components::{forms, icons};
use crate::tauri_invoke;
use yew::platform::spawn_local;
use yew::prelude::*;

use shared::{
    event::ClientInvoke,
    form::{FormType, SettingOpts},
    response::DefaultIndices,
};
use yew::virtual_dom::VNode;

#[derive(Properties, PartialEq)]
pub struct IndexFilesHelpProps {
    #[prop_or_default]
    pub toggle_file_indexer: bool,
    #[prop_or_default]
    pub onchange: Callback<SettingChangeEvent>,
}

#[function_component(IndexFilesHelp)]
pub fn index_files_help(props: &IndexFilesHelpProps) -> Html {
    let paths: UseStateHandle<Vec<String>> = use_state(Vec::new);
    {
        let paths_state = paths.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    match tauri_invoke::<_, DefaultIndices>(ClientInvoke::DefaultIndices, "").await
                    {
                        Ok(result) => {
                            let mut sorted = result.file_paths;
                            sorted.sort();
                            paths_state
                                .set(sorted.iter().map(|p| p.display().to_string()).collect());
                        }
                        Err(err) => {
                            log::info!("error: {}", err);
                        }
                    }
                });

                || ()
            },
            (),
        );
    }

    let toggle = SettingOpts {
        label: "Enable local file searching".into(),
        value: serde_json::to_string(&props.toggle_file_indexer).unwrap_or_default(),
        form_type: FormType::Bool,
        help_text: None,
    };

    let paths_rendered: VNode = paths
        .iter()
        .map(|p| {
            html! { <li class="list-disc">{p}</li> }
        })
        .collect();

    html! {
        <div class="p-4 bg-neutral-800 h-screen text-left text-neutral-400 flex flex-col gap-4">
            <h1 class="text-2xl flex flex-row items-center gap-2 text-white">
                <icons::FileExtIcon class="w-8 h-8" ext="any" />
                <div>{"Search your local files"}</div>
            </h1>
            <div class="text-sm">
                {"Enable local file search to index & search through markdown, word, excel, and other text based documents!"}
            </div>
            <div class="my-4">
                <forms::FormElement
                    class="flex flex-row"
                    setting_name="_.file-indexer"
                    opts={toggle}
                    onchange={props.onchange.clone()}
                />
            </div>
            <div class="text-sm">
                {"If enabled, the following folders will be automatically indexed. You can add/remove folders in your settings."}
                <ul class="mt-4 text-sm text-cyan-500 flex flex-col gap-2 font-mono">
                    {paths_rendered}
                </ul>
            </div>
        </div>
    }
}

#[function_component(IndexCloudHelp)]
pub fn index_cloud_help() -> Html {
    html! {
        <div class="p-4 bg-neutral-800 h-screen text-left text-neutral-400 flex flex-col gap-4">
            <h1 class="text-2xl flex flex-row items-center gap-2 text-white">
                <icons::ShareIcon height="h-8" width="w-8" />
                <div>{"Search your cloud accounts"}</div>
            </h1>
            <div class="text-sm">
                {"Add accounts in the "}
                <span class="font-bold text-cyan-500">{"Connections"}</span>
                {" tab to search through your Google Drive, Reddit posts, GitHub repos, and more!"}
            </div>
            <div>
                <img src="/connections-tab.png" class="w-[300px] mx-auto rounded shadow-md shadow-cyan-500/50" />
            </div>
        </div>
    }
}

#[function_component(IndexWebHelp)]
pub fn index_web_help() -> Html {
    html! {
        <div class="p-4 bg-neutral-800 h-screen text-left text-neutral-400 flex flex-col gap-4">
            <h1 class="text-2xl flex flex-row items-center gap-2 text-white">
                <icons::GlobeIcon width="w-8" height="h-8" />
                <div>{"Search web context"}</div>
            </h1>
            <div class="text-sm">
                {"Add lenses from the "}
                <span class="font-bold text-cyan-500">{"Discover"}</span>
                {" tab to begin searching your favorite web content instantly."}
            </div>
            <div>
                <img src="/discover-tab.png"  class="w-[300px] mx-auto rounded shadow-md shadow-cyan-500/50"/>
            </div>
        </div>
    }
}
