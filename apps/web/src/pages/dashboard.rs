use ui_components::btn::{Btn, BtnSize, BtnType};
use ui_components::icons;
use yew::{platform::spawn_local, prelude::*};
use yew_router::prelude::use_navigator;

use crate::components::LensList;
use crate::{client::Lens, AuthStatus, Route};

#[derive(Properties, PartialEq)]
pub struct DashboardProps {
    pub session_uuid: String,
    #[prop_or_default]
    pub on_create_lens: Callback<Lens>,
    #[prop_or_default]
    pub on_select_lens: Callback<Lens>,
    #[prop_or_default]
    pub on_edit_lens: Callback<Lens>,
}

#[function_component(Dashboard)]
pub fn landing_page(props: &DashboardProps) -> Html {
    let navigator = use_navigator().expect("Navigator not available");
    let auth_status = use_context::<AuthStatus>().expect("ctx not setup");

    let user_data = auth_status.user_data.clone();

    let create_lens_cb = {
        let auth_status_handle = auth_status;
        let on_create = props.on_create_lens.clone();
        Callback::from(move |_: MouseEvent| {
            let navigator = navigator.clone();
            let auth_status_handle: AuthStatus = auth_status_handle.clone();
            let on_create = on_create.clone();
            spawn_local(async move {
                // create a new lens
                let api = auth_status_handle.get_client();
                match api.lens_create().await {
                    Ok(new_lens) => {
                        on_create.emit(new_lens.clone());
                        navigator.push(&Route::Edit {
                            lens: new_lens.name,
                        })
                    }
                    Err(err) => log::error!("error creating lens: {err}"),
                }
            });
        })
    };

    if let Some(user_data) = user_data {
        html! {
            <div class="p-8">
                <div class="flex flex-row items-center mb-2 justify-between">
                    <div class="uppercase text-xs text-gray-500 font-bold">
                        {"My Lenses"}
                    </div>
                    <Btn size={BtnSize::Xs} _type={BtnType::Primary} onclick={create_lens_cb.clone()}>
                        <icons::PlusIcon width="w-4" height="h-4" />
                        <span>{"Create New"}</span>
                    </Btn>
                </div>
                <LensList
                    class="text-sm"
                    lenses={user_data.lenses.clone()}
                    on_select={props.on_select_lens.clone()}
                    on_edit={props.on_edit_lens.clone()}
                />
            </div>
        }
    } else {
        html! {}
    }
}

#[derive(Properties, PartialEq)]
struct PublicExampleProps {
    href: String,
    name: String,
    description: String,
    sources: Vec<String>,
}

#[function_component(PublicExample)]
fn pub_example(props: &PublicExampleProps) -> Html {
    let sources = props
        .sources
        .iter()
        .map(|source| {
            html! {
                <span class="ml-2 underline text-cyan-500">{source}</span>
            }
        })
        .collect::<Html>();

    html! {
        <a
            href={props.href.clone()}
            class="flex flex-col justify-between border border-neutral-600 p-4 rounded-md hover:border-cyan-500 cursor-pointer"
        >
            <div class="pb-2">{props.name.clone()}</div>
            <div class="text-sm text-neutral-400">{props.description.clone()}</div>
            <div class="pt-4 text-xs mt-auto">
                <span class="text-neutral-400">{"source:"}</span>
                {sources}
            </div>
        </a>
    }
}
