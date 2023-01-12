use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::{icons, tooltip::Tooltip};

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

#[derive(Properties, PartialEq)]
pub struct DeleteDomainButtonProps {
    pub domain: String,
    pub ondelete: Option<Callback<MouseEvent>>,
}

#[function_component(DeleteDomainButton)]
pub fn delete_button(props: &DeleteDomainButtonProps) -> Html {
    let onclick = {
        let domain = props.domain.clone();
        let callback = props.ondelete.clone();

        Callback::from(move |me| {
            let domain = domain.clone();
            let callback = callback.clone();

            spawn_local(async move {
                let _ = crate::delete_domain(domain.clone()).await;
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
            <icons::TrashIcon height={"h-4"} width={"w-4"} />
            <span class="pl-1">{"Delete"}</span>
        </button>
    }
}

#[derive(PartialEq, Eq)]
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
    pub onclick: Callback<MouseEvent>,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub href: String,
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
            "active:bg-neutral-700"
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

    if props.href.is_empty() {
        html! {
            <button onclick={props.onclick.clone()} class={styles} disabled={props.disabled}>
                <div class="flex flex-row gap-1 items-center">
                    {props.children.clone()}
                </div>
            </button>
        }
    } else {
        html! {
            <a onclick={props.onclick.clone()} href={props.href.clone()} class={styles} target="_blank">
                <div class="flex flex-row gap-1 items-center">
                    {props.children.clone()}
                </div>
            </a>
        }
    }
}
