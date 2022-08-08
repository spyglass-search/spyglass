use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::icons;

#[derive(Properties, PartialEq)]
pub struct TooltipProps {
    pub label: String,
}

#[function_component(Tooltip)]
pub fn tooltip(props: &TooltipProps) -> Html {
    let styles = vec![
        "group-hover:block",
        "group-hover:text-neutral-400",
        "py-1",
        "px-2",
        // Positioning
        "-ml-16",
        "-mt-2",
        "rounded",
        "hidden",
        "absolute",
        "text-center",
        "bg-neutral-900",
        "text-sm",
        "text-right",
    ];

    html! {
        <div class={styles}>
            {props.label.clone()}
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct DeleteButtonProps {
    pub doc_id: String,
}

#[function_component(DeleteButton)]
pub fn delete_btn(props: &DeleteButtonProps) -> Html {
    let onclick = {
        let doc_id = props.doc_id.clone();
        Callback::from(move |_| {
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

#[derive(Properties, PartialEq)]
pub struct DefaultBtnProps {
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
    let styles = classes!(
        props.classes.clone(),
        "cursor-pointer",
        "border-neutral-600",
        "border",
        "flex-row",
        "flex",
        "p-2",
        "rounded-lg",
        "text-sm",
        if props.disabled {
            classes!("text-stone-700")
        } else {
            classes!("hover:bg-neutral-600", "active:bg-neutral-700")
        },
    );

    if props.href.is_empty() {
        html! {
            <button onclick={props.onclick.clone()} class={styles} disabled={props.disabled}>
                {props.children.clone()}
            </button>
        }
    } else {
        html! {
            <a onclick={props.onclick.clone()} href={props.href.clone()} class={styles} target="_blank">
                {props.children.clone()}
            </a>
        }
    }
}
