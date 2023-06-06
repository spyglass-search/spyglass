use ui_components::{
    btn::{Btn, BtnSize, BtnType},
    icons,
};
use yew::{platform::spawn_local, prelude::*};
use yew_router::prelude::use_navigator;

use crate::{client::Lens, AuthStatus, Route};

pub mod chat_bubble;
pub mod nav;

#[derive(Properties, PartialEq)]
pub struct LensListProps {
    pub lenses: Option<Vec<Lens>>,
    #[prop_or_default]
    pub on_select: Callback<Lens>,
    #[prop_or_default]
    pub on_edit: Callback<Lens>,
    #[prop_or_default]
    pub on_delete: Callback<Lens>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(LensList)]
pub fn lens_list(props: &LensListProps) -> Html {
    let navigator = use_navigator().unwrap();
    let is_deleting = use_state_eq(|| false);
    let auth_status = use_context::<AuthStatus>().expect("Ctxt not set up");
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

        let on_delete = {
            let status = auth_status.clone();
            let lens = lens.clone();
            let is_deleting = is_deleting.clone();
            let on_delete_callback = props.on_delete.clone();
            Callback::from(move |e: MouseEvent| {
                e.stop_immediate_propagation();
                let client = status.get_client();
                let lens = lens.clone();
                let is_deleting = is_deleting.clone();
                let on_delete_callback = on_delete_callback.clone();
                spawn_local(async move {
                    is_deleting.set(true);
                    let _ = client.lens_delete(&lens.name).await;
                    is_deleting.set(false);
                    on_delete_callback.emit(lens);
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
                <>
                <Btn size={BtnSize::Sm} classes="rounded" onclick={on_edit}>
                    <icons::PencilIcon height="h-3" width="w-3" />
                    <span>{"Edit"}</span>
                </Btn>
                <Btn size={BtnSize::Sm}  _type={BtnType::Danger} classes="rounded" disabled={*is_deleting} onclick={on_delete}>
                    {if *is_deleting {
                        html! {<icons::RefreshIcon height="h-3" width="h-3" animate_spin={true} />}
                    } else {
                        html! { <icons::TrashIcon height="h-3" width="w-3" /> }
                    }}
                    <span>{"Delete"}</span>
                </Btn>
                </>
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
