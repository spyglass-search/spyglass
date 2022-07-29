use crate::components::Header;
use yew::prelude::*;

#[function_component(UserSettingsPage)]
pub fn user_settings_page() -> Html {
    html! {
        <div class="text-white">
            <Header label="User Settings" />
            <div>{"Hello world"}</div>
        </div>
    }
}
