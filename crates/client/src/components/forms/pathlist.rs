use std::path::{Path, PathBuf};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use super::FormFieldProps;
use crate::components::forms::SettingChangeEvent;
use crate::components::{btn, icons};
use crate::{invoke, listen, open_folder_path};
use shared::event::{ClientEvent, ClientInvoke, ListenPayload};

#[derive(Debug, Clone)]
pub enum PathMsg {
    UpdatePath(PathBuf),
    OpenPath(PathBuf),
    OpenFolderDialog,
}

pub struct PathField {
    pub path: PathBuf,
    pub listen_for_change: bool,
}

impl PathField {
    pub fn emit_onchange(&self, ctx: &Context<Self>) {
        let props = ctx.props();
        props.onchange.emit(SettingChangeEvent {
            setting_name: props.name.clone(),
            new_value: self.path.display().to_string(),
            restart_required: props.restart_required,
        });
    }
}

impl Component for PathField {
    type Message = PathMsg;
    type Properties = FormFieldProps;

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();
        let props = ctx.props();

        // Listen for new folder paths chosen
        {
            let link = link.clone();
            spawn_local(async move {
                let cb = Closure::wrap(Box::new(move |payload: JsValue| {
                    if let Ok(res) =
                        serde_wasm_bindgen::from_value::<ListenPayload<String>>(payload)
                    {
                        link.send_message(PathMsg::UpdatePath(
                            Path::new(&res.payload).to_path_buf(),
                        ));
                    }
                }) as Box<dyn Fn(JsValue)>);

                let _ = listen(ClientEvent::FolderChosen.as_ref(), &cb).await;
                cb.forget();
            });
        }

        Self {
            path: Path::new(&props.value).to_path_buf(),
            listen_for_change: false,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            PathMsg::UpdatePath(path) => {
                if self.listen_for_change {
                    self.listen_for_change = false;
                    self.path = path;
                    self.emit_onchange(ctx);
                    true
                } else {
                    false
                }
            }
            PathMsg::OpenPath(path) => {
                spawn_local(async move {
                    let _ = open_folder_path(path.display().to_string()).await;
                });

                false
            }
            PathMsg::OpenFolderDialog => {
                self.listen_for_change = true;
                spawn_local(async {
                    let _ = invoke(ClientInvoke::ChooseFolder.as_ref(), JsValue::NULL).await;
                });

                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let path = self.path.clone();
        let open_msg = PathMsg::OpenPath(path.clone());

        let path_html = if path.display().to_string().is_empty() {
            html! {
                <></>
            }
        } else {
            html! {
                <div class="border-1 rounded-md bg-stone-700 p-2">
                    <div class="flex items-center">
                        <button class={classes!("flex-none", "mr-2", "group")} onclick={link.callback(move |_| open_msg.clone())} >
                            <icons::FolderIcon
                                height="h-5"
                                width="w-5"
                                classes={classes!("stroke-slate-400")}
                            />
                        </button>
                        <div class={classes!("grow", "text-sm")}>{path.display().to_string()}</div>
                    </div>
                </div>
            }
        };

        html! {
            <div class="flex flex-col gap-4">
                {path_html}
                <div>
                    <btn::Btn onclick={link.callback(|_| PathMsg::OpenFolderDialog)}>
                        <icons::FolderPlusIcon classes="mr-2" />
                        {"Choose Folder"}
                    </btn::Btn>
                </div>
            </div>
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    AddPath(PathBuf),
    OpenFolderDialog,
    OpenPath(PathBuf),
    RemovePath(PathBuf),
}

/// A variation of a string list that opens a native folder chooser dialog to add
/// a new value to the list.
pub struct PathList {
    pub paths: Vec<PathBuf>,
    pub listen_for_change: bool,
}

impl PathList {
    pub fn emit_onchange(&self, ctx: &Context<Self>) {
        let props = ctx.props();

        if let Ok(new_value) = serde_json::to_string(&self.paths) {
            props.onchange.emit(SettingChangeEvent {
                setting_name: props.name.clone(),
                new_value,
                restart_required: props.restart_required,
            });
        }
    }
}

impl Component for PathList {
    type Message = Msg;
    type Properties = FormFieldProps;

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();
        let props = ctx.props();

        // Listen for new folder paths chosen
        {
            let link = link.clone();
            spawn_local(async move {
                let cb = Closure::wrap(Box::new(move |payload: JsValue| {
                    if let Ok(res) =
                        serde_wasm_bindgen::from_value::<ListenPayload<String>>(payload)
                    {
                        link.send_message(Msg::AddPath(Path::new(&res.payload).to_path_buf()));
                    }
                }) as Box<dyn Fn(JsValue)>);

                let _ = listen(ClientEvent::FolderChosen.as_ref(), &cb).await;
                cb.forget();
            });
        }

        let mut paths =
            serde_json::from_str::<Vec<PathBuf>>(&props.value).map_or(Vec::new(), |x| x);
        paths.sort();

        Self {
            paths,
            listen_for_change: false,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::AddPath(path) => {
                if self.listen_for_change {
                    self.listen_for_change = false;
                    self.paths.push(path);
                    self.paths.sort();
                    self.emit_onchange(ctx);
                    true
                } else {
                    false
                }
            }
            Msg::OpenFolderDialog => {
                self.listen_for_change = true;
                spawn_local(async {
                    let _ = invoke(ClientInvoke::ChooseFolder.as_ref(), JsValue::NULL).await;
                });

                false
            }
            Msg::OpenPath(path) => {
                spawn_local(async move {
                    let _ = open_folder_path(path.display().to_string()).await;
                });

                false
            }
            Msg::RemovePath(path) => {
                self.paths.retain(|s| **s != path);
                self.emit_onchange(ctx);

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
                <div class="flex items-center p-1.5">
                    <button class={classes!("flex-none", "mr-2", "group")} onclick={link.callback(move |_| open_msg.clone())} >
                        <icons::FolderIcon
                            height="h-5"
                            width="w-5"
                            classes={classes!("stroke-slate-400")}
                        />
                    </button>
                    <div class={classes!("grow", "text-sm")}>{path.display().to_string()}</div>
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
                <div class="border-1 rounded-md bg-stone-700 p-2 h-40 overflow-y-auto">
                    {paths_html}
                </div>
                <div class="mt-4">
                    <btn::Btn onclick={link.callback(|_| Msg::OpenFolderDialog)}>
                        <icons::FolderPlusIcon classes="mr-2" />
                        {"Add Folder"}
                    </btn::Btn>
                </div>
            </div>
        }
    }
}
