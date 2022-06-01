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

fn handle_delete(doc_id: String) {
    spawn_local(async move {
        let _ = crate::delete_doc(doc_id.clone()).await;
    });
}

#[function_component(DeleteButton)]
pub fn delete_btn(props: &DeleteButtonProps) -> Html {
    let onclick = {
        let doc_id = props.doc_id.clone();
        move |_| {
            handle_delete(doc_id.clone());
        }
    };

    html! {
        <button
            {onclick}
            class="hover:text-red-600 text-neutral-600 group">
            <Tooltip label={"Delete"} />
            <icons::TrashIcon size={4} />
        </button>
    }
}
