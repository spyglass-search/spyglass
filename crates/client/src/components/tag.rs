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
        "lens" => html! { <icons::SearchIcon height="h-4" width="w-4" classes="m-0.5" /> },
        _ => html!{ <small class="py-0.5 px-1">{props.label.clone()}</small> },
    };

    html! {
        <div class="text-xs flex flex-row rounded text-white bg-cyan-600 items-center">
            <div class="border-r border-cyan-900">
                {tag_label}
            </div>
            <div class="py-0.5 px-1">
                {props.value.clone()}
            </div>
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
