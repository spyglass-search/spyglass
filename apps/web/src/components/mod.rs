use ui_components::{
    btn::{Btn, BtnSize, BtnType},
    icons,
};
use yew::prelude::*;
use yew_router::prelude::use_navigator;

use crate::{client::Lens, Route};

pub mod nav;

#[derive(Properties, PartialEq)]
pub struct LensListProps {
    pub current: Option<String>,
    pub lenses: Option<Vec<Lens>>,
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
        "text-sm"
    );

    let current_lens = props.current.clone().unwrap_or_default();
    let mut html = Vec::new();
    let lenses = props.lenses.clone();
    for lens in lenses.unwrap_or_default() {
        let classes = classes!(
            default_classes.clone(),
            if current_lens == lens.name {
                Some("bg-cyan-800")
            } else {
                None
            }
        );

        let navi = navigator.clone();
        let lens_name = lens.name.clone();
        let onclick = Callback::from(move |_| {
            navi.push(&Route::Search {
                lens: lens_name.clone(),
            })
        });

        let icon = if lens.is_public {
            html! { <icons::GlobeIcon classes="mr-2" height="h-3" width="w-3" /> }
        } else {
            html! { <icons::CollectionIcon classes="mr-2" height="h-3" width="w-3" /> }
        };

        let navi = navigator.clone();
        let lens_name = lens.name.clone();
        let on_edit = Callback::from(move |e: MouseEvent| {
            e.stop_immediate_propagation();
            navi.push(&Route::Edit {
                lens: lens_name.clone(),
            })
        });

        let edit_icon = if lens.is_public {
            html! {}
        } else {
            html! {
                <Btn size={BtnSize::Sm} _type={BtnType::Borderless} classes="ml-auto" onclick={on_edit}>
                    <icons::PencilIcon height="h-3" width="w-3" />
                </Btn>
            }
        };

        html.push(html! {
            <li class="mb-1 flex flex-row items-center">
                <a class={classes.clone()} {onclick}>
                    {icon}
                    {lens.display_name.clone()}
                    {edit_icon}
                </a>
            </li>
        });
    }

    html! { <ul>{html}</ul> }
}
