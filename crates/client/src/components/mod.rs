pub mod btn;
pub mod forms;
pub mod icons;
pub mod result;
pub mod tooltip;

use yew::{prelude::*, virtual_dom::AttrValue};

#[derive(Properties, PartialEq, Eq)]
pub struct SelectLensProps {
    pub lens: Vec<String>,
}

/// Render a list of selected lenses
#[function_component(SelectedLens)]
pub fn selected_lens_list(props: &SelectLensProps) -> Html {
    let items = props
        .lens
        .iter()
        .map(|lens_name: &String| {
            html! {
                <li class="flex bg-cyan-700 rounded-lg my-3 ml-3">
                    <span class="text-4xl text-white p-3">{lens_name}</span>
                </li>
            }
        })
        .collect::<Html>();

    html! {
        <ul class="flex bg-neutral-800">
            {items}
        </ul>
    }
}

#[derive(PartialEq, Properties)]
pub struct HeaderProps {
    pub label: AttrValue,
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub classes: Classes,
    #[prop_or_default]
    pub tabs: Html,
    #[prop_or_default]
    pub icon: Html,
}

#[function_component(Header)]
pub fn header(props: &HeaderProps) -> Html {
    html! {
        <div class={classes!(props.classes.clone(), "px-8", "top-0", "sticky", "bg-stone-800", "z-50", "border-b-2", "border-stone-900")}>
            <div class="flex flex-row items-center gap-4">
                <h1 class="text-xl grow flex flex-row items-center py-6">
                    {props.icon.clone()}
                    {props.label.clone()}
                </h1>
                {props.children.clone()}
            </div>
            <div class="flex flex-row items-center gap-4">
                {props.tabs.clone()}
            </div>
        </div>
    }
}

#[derive(Debug)]
pub struct TabEvent {
    pub tab_idx: usize,
    pub tab_name: String,
}

#[derive(PartialEq, Properties)]
pub struct TabsProps {
    #[prop_or_default]
    pub onchange: Callback<TabEvent>,
    pub tabs: Vec<String>,
}

#[function_component(Tabs)]
pub fn tabs(props: &TabsProps) -> Html {
    let active_idx = use_state_eq(|| 0);
    let tab_styles = classes!(
        "block",
        "border-b-2",
        "px-4",
        "py-2",
        "text-xs",
        "font-medium",
        "uppercase",
        "hover:bg-stone-700",
        "hover:border-green-500",
    );

    let onchange = props.onchange.clone();
    let tabs = props.tabs.clone();
    use_effect_with_deps(
        move |updated| {
            onchange.emit(TabEvent {
                tab_idx: **updated,
                tab_name: tabs[**updated].clone(),
            });
            || {}
        },
        active_idx.clone(),
    );

    html! {
        <ul class="flex flex-row list-none gap-4">
        {
            props.tabs.iter().enumerate().map(|(idx, tab_name)| {
                let border = if idx == *active_idx { "border-green-500" } else { "border-transparent" };
                let active_idx = active_idx.clone();
                let onclick = Callback::from(move |_| {
                    active_idx.set(idx);
                });

                html! {
                    <li>
                        <button
                            onclick={onclick}
                            class={classes!(tab_styles.clone(), border)}
                        >
                            {tab_name}
                        </button>
                    </li>
                }
            })
            .collect::<Html>()
        }
        </ul>
    }
}
