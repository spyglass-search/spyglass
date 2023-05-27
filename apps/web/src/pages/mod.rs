use yew::prelude::*;

pub mod dashboard;
pub mod landing;
pub mod lens_editor;
pub mod search;

#[derive(Properties, PartialEq)]
pub struct AppPageProps {
    #[prop_or_default]
    pub children: Children,
}

#[function_component]
pub fn AppPage(props: &AppPageProps) -> Html {
    html! {
        <div class="flex-col flex-1 min-h-screen">
            {props.children.clone()}
        </div>
    }
}
