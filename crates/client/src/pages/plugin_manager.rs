use shared::event::ClientEvent;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::function_component;
use yew::prelude::*;

use shared::event::ClientInvoke;
use shared::response::PluginResult;

use crate::components::forms::Toggle;
use crate::components::{icons, Header};
use crate::utils::RequestState;
use crate::{invoke, listen, toggle_plugin};

#[derive(Properties, PartialEq, Eq)]
pub struct PluginProps {
    pub plugin: PluginResult,
}

#[function_component(Plugin)]
pub fn plugin_comp(props: &PluginProps) -> Html {
    let plugin = &props.plugin;
    let component_styles: Classes = classes!("py-4", "px-8", "flex", "flex-row", "items-center");

    let onclick = {
        let plugin_name = plugin.title.clone();
        Callback::from(move |_| {
            let plugin_name = plugin_name.clone();
            spawn_local(async move {
                if let Err(e) = toggle_plugin(&plugin_name).await {
                    log::error!("Error toggling plugin: {:?}", e);
                }
            })
        })
    };

    html! {
        <div class={component_styles}>
            <div>
                <h2 class="text-xl truncate p-0">
                    {plugin.title.clone()}
                </h2>
                <h2 class="text-xs truncate py-1 text-neutral-400">
                    {"Crafted By:"}
                    <span class="ml-2 text-cyan-400">{plugin.author.clone()}</span>
                </h2>
                <div class="text-sm leading-relaxed text-neutral-400">
                    {plugin.description.clone()}
                </div>
            </div>
            <div class="ml-auto grow">
                <Toggle
                    name={format!("{}-toggle", plugin.title)}
                    value={serde_json::to_string(&plugin.is_enabled).expect("Unable to serialize")}
                    onchange={onclick}
                />
            </div>
        </div>
    }
}

pub enum Msg {
    FetchPlugins,
    SetError(String),
    SetPlugins(Vec<PluginResult>),
}

pub struct PluginManagerPage {
    plugins: Vec<PluginResult>,
    error_msg: Option<String>,
    req_state: RequestState,
}

impl Component for PluginManagerPage {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();
        link.send_message(Msg::FetchPlugins);

        // Listen for updates from plugins
        {
            let link = link.clone();
            spawn_local(async move {
                let cb = Closure::wrap(Box::new(move |_| {
                    link.send_message(Msg::FetchPlugins);
                }) as Box<dyn Fn(JsValue)>);

                let _ = listen(ClientEvent::RefreshPluginManager.as_ref(), &cb).await;
                cb.forget();
            });
        }

        Self {
            error_msg: None,
            plugins: Vec::new(),
            req_state: RequestState::NotStarted,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let link = ctx.link();

        match msg {
            Msg::FetchPlugins => {
                self.req_state = RequestState::InProgress;
                link.send_future(async {
                    match invoke(ClientInvoke::ListPlugins.as_ref(), JsValue::NULL).await {
                        Ok(results) => match serde_wasm_bindgen::from_value(results) {
                            Ok(plugins) => Msg::SetPlugins(plugins),
                            Err(e) => Msg::SetError(format!("Error fetching plugins: {e:?}")),
                        },
                        Err(e) => Msg::SetError(format!("Error fetching plugins: {e:?}")),
                    }
                });
                false
            }
            Msg::SetError(msg) => {
                log::error!("SetError: {}", msg);
                self.req_state = RequestState::Error;
                self.error_msg = Some(msg);
                true
            }
            Msg::SetPlugins(plugins) => {
                self.req_state = RequestState::Finished;
                self.plugins = plugins;
                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let contents = if let Some(msg) = &self.error_msg {
            html! {
                <div class="flex justify-center">
                    <div class="p-16">
                        <icons::EmojiSadIcon height={"h-16"} width={"w-16"} animate_spin={true} />
                        <div>{msg}</div>
                    </div>
                </div>
            }
        } else if self.req_state.is_done() {
            self.plugins
                .iter()
                .map(|plugin| html! { <Plugin plugin={plugin.clone()} /> })
                .collect::<Html>()
        } else {
            html! {
                <div class="flex justify-center">
                    <div class="p-16">
                        <icons::RefreshIcon height={"h-16"} width={"w-16"} animate_spin={true} />
                    </div>
                </div>
            }
        };

        html! {
            <div>
                <Header label="Plugins" />
                <div>{contents}</div>
            </div>
        }
    }
}
