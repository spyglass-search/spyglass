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
        "lens" => "🔍",
        _ => &props.label,
    };

    html! {
        <div class="text-xs flex flex-row rounded text-white bg-cyan-600 items-center">
            <div class="border-r border-cyan-900 py-0.5 px-1">
                <small>{tag_label}</small>
            </div>
            <div class="py-0.5 px-2">
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
