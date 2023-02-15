use super::icons;
use yew::prelude::*;

#[derive(Properties, PartialEq, Eq)]
pub struct TagProps {
    pub label: String,
    pub value: String,
}

#[function_component(Tag)]
pub fn tag_component(props: &TagProps) -> Html {
    let tag_label = match props.label.as_str() {
        "lens" => html! { <icons::SearchIcon height="h-4" width="w-4" /> },
        _ => html! { <>{props.label.clone()}</> },
    };

    html! {
        <div class="flex flex-row rounded border border-neutral-600 gap-1 py-0.5 px-1 text-xs text-white">
            <div class="font-bold text-cyan-600">{tag_label}</div>
            <div>{props.value.clone()}</div>
        </div>
    }
}

#[function_component(TagIcon)]
pub fn tag_icon_component(_props: &TagProps) -> Html {
    html! {
        <div class="items-center">
            <icons::StarIcon height={"h-4"} width={"w-4"} classes="text-yellow-500" />
        </div>
    }
}
