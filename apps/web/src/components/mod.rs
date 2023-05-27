use ui_components::{
    btn::{Btn, BtnSize},
    icons,
};
use yew::prelude::*;
use yew_router::prelude::use_navigator;

use crate::{client::Lens, Route};

pub mod nav;

#[derive(Properties, PartialEq)]
pub struct LensListProps {
    pub lenses: Option<Vec<Lens>>,
    #[prop_or_default]
    pub on_select: Callback<Lens>,
    #[prop_or_default]
    pub on_edit: Callback<Lens>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(LensList)]
pub fn lens_list(props: &LensListProps) -> Html {
    let navigator = use_navigator().unwrap();
    let default_classes = classes!(
        "hover:bg-cyan-600",
        "cursor-pointer",
        "flex",
        "flex-grow",
        "flex-row",
        "items-center",
        "py-1.5",
        "px-2",
        "rounded",
        "overflow-hidden",
        "whitespace-nowrap",
        "text-ellipsis",
        props.class.clone(),
    );

    let mut html = Vec::new();
    let lenses = props.lenses.clone();
    for lens in lenses.unwrap_or_default() {
        let classes = classes!(default_classes.clone(),);

        let onclick = {
            let navi = navigator.clone();
            let lens = lens.clone();
            let on_select = props.on_select.clone();

            Callback::from(move |_| {
                on_select.emit(lens.clone());
                navi.push(&Route::Search {
                    lens: lens.name.clone(),
                })
            })
        };

        let on_edit = {
            let navi = navigator.clone();
            let lens = lens.clone();
            let on_edit = props.on_edit.clone();

            Callback::from(move |e: MouseEvent| {
                e.stop_immediate_propagation();
                on_edit.emit(lens.clone());
                navi.push(&Route::Edit {
                    lens: lens.name.clone(),
                })
            })
        };

        let icon = if lens.is_public {
            html! { <icons::GlobeIcon classes="mr-2 flex-none" height="h-3" width="w-3" /> }
        } else {
            html! { <icons::CollectionIcon classes="mr-2 flex-none" height="h-3" width="w-3" /> }
        };

        let edit_icon = if lens.is_public {
            html! {}
        } else {
            html! {
                <Btn size={BtnSize::Sm} classes="rounded" onclick={on_edit}>
                    <icons::PencilIcon height="h-3" width="w-3" />
                    <span>{"Edit"}</span>
                </Btn>
            }
        };

        html.push(html! {
            <li class="flex flex-row items-center justify-between gap-4">
                <a class={classes.clone()} {onclick}>
                    {icon}
                    <div class="truncate text-ellipsis text-lg">{lens.display_name.clone()}</div>
                </a>
                {edit_icon}
            </li>
        });
    }

    html! { <ul class="flex flex-col gap-2">{html}</ul> }
}
