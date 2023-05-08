use yew::prelude::*;

pub mod lens_edit;
pub mod search;

#[derive(Properties, PartialEq)]
pub struct AppPageProps {
    #[prop_or_default]
    pub children: Children,
}

#[function_component]
pub fn AppPage(props: &AppPageProps) -> Html {
    html! {
        <div class="flex-col flex-1 h-screen overflow-y-auto bg-neutral-800">
            {props.children.clone()}
        </div>
    }
}
