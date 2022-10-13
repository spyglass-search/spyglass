use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::{btn, icons};
use crate::{invoke, open_folder_path};
use shared::event::ClientInvoke;

#[derive(Properties, PartialEq)]
pub struct PathListProps {
    pub value: String,
    pub oninput: Callback<InputEvent>,
}

pub struct PathList {
    pub paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Add,
    OpenPath(String),
    RemovePath(String),
}

impl Component for PathList {
    type Message = Msg;
    type Properties = PathListProps;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let mut paths = serde_json::from_str::<Vec<String>>(&props.value).map_or(Vec::new(), |x| x);
        paths.sort();

        Self { paths }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        log::info!("msg recv: {:?}", msg);
        match msg {
            Msg::Add => {
                spawn_local(async {
                    let res = invoke(ClientInvoke::ChooseFolder.as_ref(), JsValue::NULL).await;
                    log::info!("{:?}", res);
                });

                false
            }
            Msg::OpenPath(path) => {
                spawn_local(async {
                    let _ = open_folder_path(path).await;
                });

                false
            }
            Msg::RemovePath(path) => {
                self.paths.retain(|s| **s != path);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let paths_html = self.paths.iter().map(|path| {
            let path = path.clone();
            let open_msg = Msg::OpenPath(path.clone());
            let rm_msg = Msg::RemovePath(path.clone());

            html!  {
                <div class="flex items-center rounded-md p-1.5">
                    <button class={classes!("flex-none", "mr-2", "group")} onclick={link.callback(move |_| open_msg.clone())} >
                        <icons::FolderIcon
                            height="h-5"
                            width="w-5"
                            classes={classes!("stroke-slate-400")}
                        />
                    </button>
                    <div class={classes!("grow", "text-sm")}>{path.clone()}</div>
                    <button class={classes!("flex-none", "group")} onclick={link.callback(move |_| rm_msg.clone())}>
                        <icons::TrashIcon
                            height="h-5"
                            width="w-5"
                            classes={classes!("stroke-slate-400", "group-hover:stroke-white", "group-hover:fill-red-400")}
                        />
                    </button>
                </div>
            }
        })
        .collect::<Html>();

        html! {
            <div>
                {paths_html}
                <div class="mt-4">
                    <btn::Btn onclick={link.callback(|_| Msg::Add)}>
                        <icons::FolderPlusIcon classes="mr-2" />
                        {"Add Folder"}
                    </btn::Btn>
                </div>
            </div>
        }
    }
}
