use super::IconProps;
use yew::function_component;
use yew::prelude::*;

#[function_component(ChromeBrowserIcon)]
pub fn chrome_browser_icon(props: &IconProps) -> Html {
    html! {
        <img src="/icons/chrome.svg" class={props.class()} />
    }
}

#[function_component(FirefoxBrowserIcon)]
pub fn firefox_browser_icon(props: &IconProps) -> Html {
    html! {
        <img src="/icons/firefox.svg" class={props.class()} />
    }
}
