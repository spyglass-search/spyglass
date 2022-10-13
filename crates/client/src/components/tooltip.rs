use yew::prelude::*;

#[derive(Properties, PartialEq, Eq)]
pub struct TooltipProps {
    pub label: String,
}

#[function_component(Tooltip)]
pub fn tooltip(props: &TooltipProps) -> Html {
    let styles = classes!(
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
        "z-50",
    );

    html! {
        <div class={styles}>
            {props.label.clone()}
        </div>
    }
}
