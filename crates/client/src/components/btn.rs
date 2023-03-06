use shared::event::OpenResultParams;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::{
    components::{icons, tooltip::Tooltip},
    tauri_invoke,
};

#[derive(Properties, PartialEq, Eq)]
pub struct DeleteButtonProps {
    pub doc_id: String,
}

#[function_component(DeleteButton)]
pub fn delete_btn(props: &DeleteButtonProps) -> Html {
    let onclick = {
        let doc_id = props.doc_id.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            e.stop_immediate_propagation();

            let doc_id = doc_id.clone();
            spawn_local(async move {
                let _ = crate::delete_doc(doc_id.clone()).await;
            });
        })
    };

    html! {
        <button
            {onclick}
            class="hover:text-red-600 text-neutral-600 group">
            <Tooltip label={"Delete"} />
            <icons::TrashIcon height={"h-4"} width={"w-4"} />
        </button>
    }
}

#[derive(Properties, PartialEq)]
pub struct RecrawlButtonProps {
    pub domain: String,
    pub onrecrawl: Option<Callback<MouseEvent>>,
}

#[function_component(RecrawlButton)]
pub fn recrawl_button(props: &RecrawlButtonProps) -> Html {
    let onclick = {
        let domain = props.domain.clone();
        let callback = props.onrecrawl.clone();

        Callback::from(move |me| {
            let domain = domain.clone();
            let callback = callback.clone();

            spawn_local(async move {
                let _ = crate::recrawl_domain(domain.clone()).await;
            });

            if let Some(callback) = callback {
                callback.emit(me);
            }
        })
    };

    html! {
        <button
            {onclick}
            class="hover:text-red-600 text-neutral-600 group flex flex-row">
            <icons::RefreshIcon height={"h-4"} width={"w-4"} />
            <span class="pl-1">{"Recrawl"}</span>
        </button>
    }
}

#[allow(dead_code)]
#[derive(Clone, PartialEq, Eq)]
pub enum BtnAlign {
    Left,
    Right,
    Center,
}

impl Default for BtnAlign {
    fn default() -> Self {
        Self::Center
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum BtnType {
    Default,
    Danger,
    Success,
}

impl Default for BtnType {
    fn default() -> Self {
        Self::Default
    }
}

#[allow(dead_code)]
#[derive(PartialEq, Eq)]
pub enum BtnSize {
    Xs,
    Sm,
    Base,
    Lg,
    Xl,
}

impl Default for BtnSize {
    fn default() -> Self {
        Self::Base
    }
}

#[derive(Properties, PartialEq)]
pub struct DefaultBtnProps {
    #[prop_or_default]
    pub _type: BtnType,
    #[prop_or_default]
    pub size: BtnSize,
    #[prop_or_default]
    pub align: BtnAlign,
    #[prop_or_default]
    pub onclick: Callback<MouseEvent>,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub href: Option<AttrValue>,
    #[prop_or_default]
    pub classes: Classes,
}

#[function_component(Btn)]
pub fn default_button(props: &DefaultBtnProps) -> Html {
    let mut colors = match props._type {
        BtnType::Default => classes!(
            "border-neutral-600",
            "border",
            "hover:bg-neutral-600",
            "active:bg-neutral-700",
            "text-white",
        ),
        BtnType::Danger => classes!(
            "border",
            "border-red-700",
            "hover:bg-red-700",
            "text-red-500",
            "hover:text-white"
        ),
        BtnType::Success => classes!("bg-green-700", "hover:bg-green-900"),
    };

    if props.disabled {
        colors.push("text-stone-400");
    }

    let sizes = match props.size {
        BtnSize::Xs => classes!("text-xs", "px-2", "py-1"),
        BtnSize::Sm => classes!("text-sm", "px-2", "py-1"),
        BtnSize::Base => classes!("text-base", "px-3", "py-2"),
        BtnSize::Lg => classes!("text-lg", "px-3", "py-2"),
        BtnSize::Xl => classes!("text-xl", "px-4", "py-4"),
    };

    let styles = classes!(
        props.classes.clone(),
        colors,
        sizes,
        "cursor-pointer",
        "flex-row",
        "flex",
        "font-semibold",
        "items-center",
        "leading-5",
        "rounded-md",
    );

    let is_confirmed = use_state(|| false);

    let confirmed_state = is_confirmed.clone();
    let prop_onclick = props.onclick.clone();
    let btn_type = props._type.clone();

    let href_prop = props.href.clone();
    let handle_onclick = Callback::from(move |evt| {
        // Handle confirmation for danger buttons
        if btn_type == BtnType::Danger {
            if *confirmed_state {
                prop_onclick.emit(evt);
            } else {
                confirmed_state.set(true);
            }
        } else {
            if let Some(href) = &href_prop {
                let href = href.clone();
                spawn_local(async move {
                    let _ = tauri_invoke::<OpenResultParams, ()>(
                        shared::event::ClientInvoke::OpenResult,
                        OpenResultParams {
                            url: href.to_string(),
                            None
                        },
                    )
                    .await;
                });
            }

            prop_onclick.emit(evt);
        }
    });

    let label = if props._type == BtnType::Danger && *is_confirmed {
        Children::new(vec![html! { <>{"⚠️ Click to confirm"}</> }])
    } else {
        props.children.clone()
    };

    let mut label_styles = classes!("flex", "flex-row", "gap-1", "items-center",);

    match &props.align {
        BtnAlign::Left => {}
        BtnAlign::Right => label_styles.push("ml-auto"),
        BtnAlign::Center => label_styles.push("mx-auto"),
    }

    if props.href.is_none() {
        html! {
            <button onclick={handle_onclick} class={styles} disabled={props.disabled}>
                <div class={label_styles}>
                    {label}
                </div>
            </button>
        }
    } else {
        html! {
            <a onclick={handle_onclick} class={styles}>
                <div class={label_styles}>
                    {label}
                </div>
            </a>
        }
    }
}
